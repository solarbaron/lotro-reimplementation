// SPDX-License-Identifier: GPL-3.0-or-later
//! Golden images of the lit pipeline (M4.5, `docs/specs/render.md` §4):
//!
//! - `lit_mesh.png`: a lit scene with the sun's shadow, a point light, hemispheric ambient,
//!   specular and distance fog (M4.5 exit criterion 7);
//! - `lit_surfaces.png`: the other lit surface states: unlit, additive, alpha test with
//!   blend on two-sided cards, translucent blend, emissive and glow, vertex colours, gloss
//!   from the base alpha, under a point light and fog.
//!
//! Every mesh comes from the renderer's procedural shapes and every texture is generated
//! here, so the images contain no game data. Regenerate with
//! `TESSERA_BLESS=1 cargo test -p tessera-render --test lit_golden` and review the PNGs in
//! `tests/golden/` (see `tests/golden/README.md` for the drivers they are checked on).

#[path = "support/golden.rs"]
mod golden;
#[path = "support/gpu.rs"]
mod gpu;

use std::f32::consts::FRAC_PI_2;
use std::path::Path;

use tessera_render::glam::{Mat4, Quat, Vec3};
use tessera_render::lighting::reference::frame_ambient;
use tessera_render::{
    AlphaMode, Camera, ColorSpace, Filter, Fog, FrameLighting, Gloss, Light, LightFields,
    LitSurface, Material, MeshData, MeshDraw, OffscreenFormat, Opacity, Perspective, RenderError,
    Renderer, RgbaImage, Specular, SunLight, SunShadow, TextureId, TextureUpload, Wrap,
};

const WIDTH: u32 = 256;
const HEIGHT: u32 = 160;
const CLEAR: [f32; 4] = [0.42, 0.47, 0.55, 1.0];
const FOG: [f32; 3] = [0.42, 0.47, 0.55];

/// `size × size` RGBA8 texture from a per-texel function of `(x, y)`.
fn texels(size: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
    (0..size * size)
        .flat_map(|i| f(i % size, i / size))
        .collect()
}

fn upload(r: &mut Renderer<'static>, size: u32, data: &[u8]) -> Result<TextureId, RenderError> {
    r.upload_texture(&TextureUpload {
        color_space: ColorSpace::Linear,
        filter: Filter::Linear,
        wrap: Wrap::Repeat,
        ..TextureUpload::rgba8(size, size, data)
    })
}

fn checker(size: u32, cells: u32, a: [u8; 4], b: [u8; 4]) -> Vec<u8> {
    let cell = (size / cells).max(1);
    texels(size, |x, y| {
        if ((x / cell) + (y / cell)).is_multiple_of(2) {
            a
        } else {
            b
        }
    })
}

fn at(x: f32, y: f32, z: f32) -> Mat4 {
    Mat4::from_translation(Vec3::new(x, y, z))
}

fn camera(eye: Vec3, target: Vec3) -> Camera {
    Camera {
        eye,
        target,
        up: Vec3::Z,
        projection: Perspective {
            fov_y: 50f32.to_radians(),
            near: 0.1,
            far: 200.0,
        },
    }
}

fn render(
    r: &mut Renderer<'static>,
    camera: &Camera,
    lighting: &FrameLighting,
    draws: &[MeshDraw],
) -> Result<RgbaImage, RenderError> {
    let target = r.create_offscreen_target(WIDTH, HEIGHT, OffscreenFormat::Rgba8Srgb)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    {
        let pass = frame
            .world_pass("lit", camera)
            .clear(CLEAR)
            .frame_lighting(lighting.clone());
        for d in draws {
            pass.mesh(*d);
        }
    }
    r.end_frame(frame)?;
    r.read_pixels(&target)
}

/// Render twice (must be bit-identical) and compare against the golden PNG.
fn check_golden(
    name: &str,
    scene: impl Fn(&mut Renderer<'static>) -> Result<RgbaImage, RenderError>,
) -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let img = scene(&mut r)?;
    let again = scene(&mut r)?;
    assert!(
        img.data == again.data,
        "{name}: render is not deterministic"
    );
    golden::check(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/golden/{name}.png")),
        &img,
        golden::Tolerance::default(),
    );
    Ok(())
}

/// Evening light: a low warm sun, sky-blue above and earth-brown below.
fn evening(shadow_center: Vec3) -> FrameLighting {
    let toward = Vec3::new(-0.55, -0.45, 0.70).normalize();
    let (ambient, hemispheric) = frame_ambient(0.55, [0.55, 0.62, 0.80], [0.30, 0.24, 0.18]);
    FrameLighting {
        up: Vec3::Z,
        ambient,
        hemispheric,
        sun: Some(SunLight {
            direction: toward,
            color: [0.42, 0.38, 0.30],
            specular: [0.8, 0.75, 0.6],
        }),
        fog: Some(Fog {
            near: 4.0,
            far: 30.0,
            color: FOG,
        }),
        shadow: Some(SunShadow::fit(toward, shadow_center, 7.0, 60.0, 1024)),
        lights: Vec::new(),
        time: 0.0,
    }
}

// ---- lit_mesh -----------------------------------------------------------------------------

fn lit_mesh_scene(r: &mut Renderer<'static>) -> Result<RgbaImage, RenderError> {
    let ground_tex = upload(
        r,
        64,
        &checker(64, 8, [0x9c, 0x94, 0x84, 0xff], [0x70, 0x6c, 0x62, 0xff]),
    )?;
    // Base alpha = gloss: shiny stripes on a matte sphere.
    let sphere_tex = upload(
        r,
        32,
        &texels(32, |_, y| {
            if (y / 4) % 2 == 0 {
                [0x50, 0x70, 0xa8, 0xff]
            } else {
                [0x50, 0x70, 0xa8, 0x30]
            }
        }),
    )?;
    let ground = r.upload_mesh(&MeshData::plane(40.0, 40.0, 8, 8))?;
    let cube = r.upload_mesh(&MeshData::cube(0.6))?;
    let sphere = r.upload_mesh(&MeshData::uv_sphere(0.7, 32, 16))?;
    let pillar = r.upload_mesh(&MeshData::tube(0.25, 2.4, 16))?;

    let center = Vec3::new(0.0, 0.0, 0.8);
    let mut lighting = evening(center);
    // A warm lamp between the sphere and the pillar, built from light fields (§8.3).
    lighting.lights.push(Light::from_fields(
        Vec3::new(1.2, -1.4, 0.6),
        &LightFields {
            diffuse: [1.0, 0.62, 0.25],
            intensity: 0.9,
            range: 4.0,
            falloff: 0.5,
            ..LightFields::default()
        },
    ));
    // A second, cold light out of reach of everything but the far floor.
    lighting.lights.push(Light::point(
        Vec3::new(-4.0, 3.0, 0.4),
        3.0,
        0.4,
        [0.3, 0.5, 1.0],
    ));

    let lit = LitSurface::default();
    let shiny = Specular {
        color: [0.9, 0.9, 0.9],
        shininess: 24.0,
        texture: None,
    };
    let draws = [
        MeshDraw::new(ground, Mat4::IDENTITY, Material::textured(ground_tex)).with_lit(
            LitSurface {
                opacity: Opacity::One,
                ..lit
            },
        ),
        MeshDraw::new(
            cube,
            at(-1.4, 0.4, 0.6) * Mat4::from_rotation_z(0.5),
            Material::color([0.85, 0.45, 0.30, 1.0]),
        ),
        MeshDraw::new(
            sphere,
            at(1.0, 0.2, 0.7),
            Material {
                specular: Some(shiny),
                ..Material::textured(sphere_tex)
            },
        )
        .with_lit(LitSurface {
            gloss: Gloss::BaseAlpha,
            opacity: Opacity::One,
            ..lit
        }),
        MeshDraw::new(
            pillar,
            at(2.6, -1.6, 1.2) * Mat4::from_rotation_x(FRAC_PI_2),
            Material {
                two_sided: true,
                specular: Some(Specular {
                    shininess: 0.25,
                    ..shiny
                }),
                ..Material::color([0.7, 0.7, 0.66, 1.0])
            },
        ),
        // Far away in the fog.
        MeshDraw::new(
            cube,
            at(-6.0, 14.0, 0.6) * Mat4::from_rotation_z(0.2),
            Material::color([0.85, 0.45, 0.30, 1.0]),
        ),
    ];
    render(
        r,
        &camera(Vec3::new(3.5, -6.5, 3.2), Vec3::new(0.0, 0.5, 0.4)),
        &lighting,
        &draws,
    )
}

#[test]
fn lit_mesh() -> Result<(), RenderError> {
    check_golden("lit_mesh", lit_mesh_scene)
}

// ---- lit_surfaces ----------------------------------------------------------------------

fn lit_surfaces_scene(r: &mut Renderer<'static>) -> Result<RgbaImage, RenderError> {
    let ground_tex = upload(
        r,
        32,
        &checker(32, 4, [0x88, 0x88, 0x80, 0xff], [0x60, 0x60, 0x5a, 0xff]),
    )?;
    // Leaf card: a disc whose alpha falls off towards the rim.
    let leaf = upload(
        r,
        32,
        &texels(32, |x, y| {
            let (dx, dy) = (x as f32 - 15.5, y as f32 - 15.5);
            let d = (dx * dx + dy * dy).sqrt() / 16.0;
            let a = ((1.0 - d) * 2.0).clamp(0.0, 1.0);
            [0x40, 0x90, 0x38, (a * 255.0) as u8]
        }),
    )?;
    let emissive_tex = upload(
        r,
        16,
        &texels(16, |x, _| {
            if x % 4 < 2 {
                [0xff, 0xff, 0xff, 0xff]
            } else {
                [0x10, 0x10, 0x10, 0xff]
            }
        }),
    )?;
    let glow_tex = upload(
        r,
        16,
        &texels(16, |_, y| {
            if y % 8 < 2 {
                [0xff; 4]
            } else {
                [0, 0, 0, 0xff]
            }
        }),
    )?;
    let ground = r.upload_mesh(&MeshData::plane(40.0, 40.0, 4, 4))?;
    let card = r.upload_mesh(&MeshData::plane(1.4, 1.4, 1, 1))?;
    let cube = r.upload_mesh(&MeshData::cube(0.45))?;
    let mut ball = MeshData::uv_sphere(0.5, 24, 12);
    ball.colors = Some(
        ball.positions
            .iter()
            .map(|p| {
                let t = (p[1] / 0.5) * 0.5 + 0.5;
                [1.0 - t, 0.4 + 0.4 * t, t, 1.0]
            })
            .collect(),
    );
    let ball = r.upload_mesh(&ball)?;
    let glow_ball = r.upload_mesh(&MeshData::uv_sphere(0.45, 24, 12))?;

    let mut lighting = evening(Vec3::new(0.0, 0.0, 0.6));
    lighting.lights.push(Light::point(
        Vec3::new(0.0, -1.2, 1.4),
        5.0,
        0.3,
        [0.9, 0.7, 0.4],
    ));
    let lit = LitSurface::default();
    let upright = Mat4::from_rotation_x(FRAC_PI_2);
    let mut draws = vec![
        MeshDraw::new(ground, Mat4::IDENTITY, Material::textured(ground_tex)).with_lit(
            LitSurface {
                opacity: Opacity::One,
                ..lit
            },
        ),
        // Unlit card (left), facing the camera.
        MeshDraw::new(
            card,
            at(-2.6, 0.5, 0.8) * upright,
            Material {
                unlit: true,
                ..Material::color([0.95, 0.85, 0.3, 1.0])
            },
        ),
    ];
    // Crossed two-sided leaf cards: alpha test then blend, casting cut-out shadows.
    for angle in [0.3f32, 0.3 + FRAC_PI_2] {
        draws.push(MeshDraw::new(
            card,
            at(-1.0, 0.8, 0.75) * Mat4::from_rotation_z(angle) * upright,
            Material {
                alpha: AlphaMode::TestBlend { cutoff: 0.3 },
                two_sided: true,
                ..Material::textured(leaf)
            },
        ));
    }
    draws.extend([
        // Emissive stripes and a glow band on a dark cube.
        MeshDraw::new(
            cube,
            at(0.4, 1.2, 0.45) * Mat4::from_rotation_z(0.6),
            Material::color([0.25, 0.25, 0.3, 1.0]),
        )
        .with_lit(LitSurface {
            glow: [0.3, 0.6, 1.0],
            emissive: Some(emissive_tex),
            glow_mask: Some(glow_tex),
            opacity: Opacity::One,
            ..lit
        }),
        // Vertex colours multiply M.
        MeshDraw::new(ball, at(1.6, 0.0, 0.5), Material::color([1.0; 4])).with_lit(LitSurface {
            vertex_colors: true,
            ..lit
        }),
        // Additive unlit halo: fades into the fog instead of turning fog coloured.
        MeshDraw::new(
            glow_ball,
            at(2.8, 2.0, 1.2),
            Material {
                alpha: AlphaMode::Additive,
                unlit: true,
                ..Material::color([1.0, 0.5, 0.15, 0.8])
            },
        ),
        // A half-transparent blue pane in front.
        MeshDraw::new(
            card,
            at(0.8, -1.4, 0.7) * Mat4::from_rotation_z(-0.4) * upright,
            Material {
                alpha: AlphaMode::Blend,
                two_sided: true,
                ..Material::color([0.3, 0.5, 0.9, 0.5])
            },
        ),
    ]);
    // Rotated copy of the leaves far away, deep in the fog.
    draws.push(MeshDraw::new(
        card,
        at(-5.0, 16.0, 0.75) * Mat4::from_quat(Quat::from_rotation_z(0.8)) * upright,
        Material {
            alpha: AlphaMode::TestBlend { cutoff: 0.3 },
            two_sided: true,
            ..Material::textured(leaf)
        },
    ));
    render(
        r,
        &camera(Vec3::new(1.5, -6.0, 2.6), Vec3::new(0.0, 0.8, 0.6)),
        &lighting,
        &draws,
    )
}

#[test]
fn lit_surfaces() -> Result<(), RenderError> {
    check_golden("lit_surfaces", lit_surfaces_scene)
}
