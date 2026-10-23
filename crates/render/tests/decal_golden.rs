// SPDX-License-Identifier: GPL-3.0-or-later
//! The placed-decal pass (`docs/specs/decals.md` §6; `docs/specs/render.md` §5) on
//! synthetic decals:
//!
//! - `decal_ground.png`: the ground decal of `decals.md` §11 example 1 (a 4 m × 6 m
//!   rectangle on a flat receiver, lifted 0.008 m, u along +X′, v along −Z′) with the
//!   colour of example 3 in mode 0 and the light *L* = (1, 1, 1, 1) of §9.2, alpha-blended
//!   with a synthetic arrow texture; beside it an additive decal partly hidden by a
//!   pillar (the depth test against the receivers), with a slope-scale depth bias;
//! - pixel checks: the alpha blend over the receiver, the texture × vertex colour ×
//!   material colour product, a vertex colour switched off (§6.1), the depth test, one draw
//!   call per batch and the decal counters.
//!
//! The decals and the texture are invented here; no game data. The decal triangles are
//! written out by hand (the builder of `tessera-world` is tested in that crate).
//! Regenerate with `TESSERA_BLESS=1 cargo test -p tessera-render --test decal_golden`.

#[path = "support/golden.rs"]
mod golden;
#[path = "support/gpu.rs"]
mod gpu;

use std::path::Path;

use tessera_render::glam::{Mat4, Vec3};
use tessera_render::{
    AlphaMode, Camera, ColorSpace, DecalDraw, DecalVertex, DrawDepthBias, Filter, FrameLighting,
    LitSurface, Material, MeshData, MeshDraw, OffscreenFormat, PassSpace, RenderError, Renderer,
    RgbaImage, SunLight, TextureId, TextureUpload, Wrap,
};

const WIDTH: u32 = 320;
const HEIGHT: u32 = 200;
const CLEAR: [f32; 4] = [0.55, 0.62, 0.72, 1.0];

/// A 64 × 64 arrow on a soft-edged disc; alpha 0 outside.
fn arrow() -> Vec<u8> {
    let n = 64u32;
    let mut out = Vec::with_capacity((n * n * 4) as usize);
    for y in 0..n {
        for x in 0..n {
            let u = (x as f32 + 0.5) / n as f32 * 2.0 - 1.0;
            let v = (y as f32 + 0.5) / n as f32 * 2.0 - 1.0;
            let r = (u * u + v * v).sqrt();
            let disc = (1.0 - r).clamp(0.0, 0.15) / 0.15;
            // Arrow pointing to the top of the image (−v): shaft and head.
            let shaft = u.abs() < 0.15 && v > -0.2 && v < 0.7;
            let head = v <= -0.2 && v > -0.75 && u.abs() < (v + 0.75) * 0.9;
            let (rgb, a) = if shaft || head {
                ([255u8, 255, 255], 1.0)
            } else {
                ([90u8, 70, 50], 0.55 * disc)
            };
            out.extend_from_slice(&[rgb[0], rgb[1], rgb[2], (a * 255.0) as u8]);
        }
    }
    out
}

/// The rectangle of example 1 moved to the origin: x ∈ [−2, 2], y ∈ [−3, 3] on the
/// receiver z = 0, lifted to 0.008; u = (x + 2) / 4, v = 0.5 − y / 6 (Z′ = +Y). Two
/// triangles, counter-clockwise from above, all with `color`.
fn ground_decal(color: [f32; 4]) -> Vec<DecalVertex> {
    let v = |x: f32, y: f32| DecalVertex {
        position: [x, y, 0.008],
        color,
        uv: [(x + 2.0) / 4.0, 0.5 - y / 6.0],
    };
    let (a, b, c, d) = (v(-2.0, -3.0), v(2.0, -3.0), v(2.0, 3.0), v(-2.0, 3.0));
    vec![a, b, c, a, c, d]
}

/// A square decal of half-width `h` at (`x`, `y`) on the receiver.
fn square(x: f32, y: f32, h: f32, color: [f32; 4]) -> Vec<DecalVertex> {
    let v = |dx: f32, dy: f32| DecalVertex {
        position: [x + dx, y + dy, 0.008],
        color,
        uv: [(dx / h + 1.0) / 2.0, (1.0 - dy / h) / 2.0],
    };
    let (a, b, c, d) = (v(-h, -h), v(h, -h), v(h, h), v(-h, h));
    vec![a, b, c, a, c, d]
}

fn texture(r: &mut Renderer<'static>) -> Result<TextureId, RenderError> {
    r.upload_texture(&TextureUpload {
        color_space: ColorSpace::Srgb,
        filter: Filter::Linear,
        wrap: Wrap::Clamp,
        ..TextureUpload::rgba8(64, 64, &arrow())
    })
}

fn scene(r: &mut Renderer<'static>) -> Result<RgbaImage, RenderError> {
    let tex = texture(r)?;
    let ground = r.upload_mesh(&MeshData::plane(16.0, 16.0, 1, 1))?;
    let pillar = r.upload_mesh(&MeshData::cube(0.5))?;
    let target = r.create_offscreen_target(WIDTH, HEIGHT, OffscreenFormat::Rgba8Srgb)?;
    let camera = Camera {
        eye: Vec3::new(1.5, -8.0, 6.0),
        target: Vec3::new(1.5, 0.0, 0.0),
        ..Camera::default()
    };
    let mut frame = r.begin_offscreen_frame(&target)?;
    // Example 3, mode 0: stored bytes (B, G, R, A) = (0, 128, 255, 204) give the vertex
    // colour (255, 128, 0, 204) / 255 whatever the light (*L* = 1, §9.2).
    let colour = [1.0, 128.0 / 255.0, 0.0, 204.0 / 255.0];
    frame
        .world_pass("decals", &camera)
        .clear(CLEAR)
        .frame_lighting(FrameLighting {
            ambient: [0.35, 0.35, 0.38],
            sun: Some(SunLight {
                direction: Vec3::new(0.3, -0.4, 0.86).normalize(),
                color: [0.6, 0.58, 0.52],
                specular: [0.0; 3],
            }),
            ..FrameLighting::default()
        })
        .mesh(
            MeshDraw::new(
                ground,
                Mat4::IDENTITY,
                Material::color([0.45, 0.47, 0.4, 1.0]),
            )
            .with_lit(LitSurface::default()),
        )
        .mesh(
            MeshDraw::new(
                pillar,
                Mat4::from_translation(Vec3::new(5.0, -1.4, 0.5))
                    * Mat4::from_scale(Vec3::new(1.0, 1.0, 2.0)),
                Material::color([0.5, 0.42, 0.38, 1.0]),
            )
            .with_lit(LitSurface::default()),
        )
        .decal(DecalDraw {
            texture: Some(tex),
            ..DecalDraw::new(ground_decal(colour))
        })
        .decal(DecalDraw {
            texture: Some(tex),
            alpha: AlphaMode::Additive,
            depth_bias: DrawDepthBias::slope(-1.0),
            ..DecalDraw::new(square(4.5, 0.5, 1.2, [0.2, 0.45, 0.9, 1.0]))
        });
    let stats = r.end_frame(frame)?;
    assert_eq!(stats.decal_draws, 2, "{stats:?}");
    assert_eq!(stats.decal_triangles, 4, "{stats:?}");
    r.read_pixels(&target)
}

#[test]
fn decal_ground_golden() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let img = scene(&mut r)?;
    let again = scene(&mut r)?;
    assert!(img.data == again.data, "render is not deterministic");
    golden::check(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/decal_ground.png"),
        &img,
        golden::Tolerance::default(),
    );
    Ok(())
}

// ---- pixel checks -------------------------------------------------------------------------

/// Orthographic top-down view of x, y in −2..2 (reversed depth).
fn top_down() -> Mat4 {
    let proj = tessera_render::glam::camera::rh::proj::directx::orthographic(
        -2.0, 2.0, -2.0, 2.0, -50.0, 50.0,
    );
    Mat4::from_cols_array(&[
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 1.0, 1.0,
    ]) * proj
}

/// A unit-colour floor at z = 0 (unlit grey), `wall` an unlit blue plane at z = 1 above
/// it when set, and the decals; returns the centre pixel and the frame's stats.
fn centre(
    r: &mut Renderer<'static>,
    decals: Vec<DecalDraw>,
    wall: bool,
) -> Result<([u8; 4], tessera_render::FrameStats), RenderError> {
    centre_in(r, decals, wall, true)
}

/// [`centre`] in a lit pass (`lit`) or a pass without `Pass::frame_lighting`.
fn centre_in(
    r: &mut Renderer<'static>,
    decals: Vec<DecalDraw>,
    wall: bool,
    lit: bool,
) -> Result<([u8; 4], tessera_render::FrameStats), RenderError> {
    let floor = r.upload_mesh(&MeshData::plane(4.0, 4.0, 1, 1))?;
    let target = r.create_offscreen_target(16, 16, OffscreenFormat::Rgba8Unorm)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    let unlit = |c: [f32; 4]| Material {
        unlit: true,
        ..Material::color(c)
    };
    let pass = frame.pass(
        "decal",
        PassSpace::World {
            view_proj: top_down(),
        },
    );
    pass.eye(Vec3::new(0.0, 0.0, 10.0))
        .clear([0.0, 0.0, 0.0, 1.0]);
    if lit {
        pass.frame_lighting(FrameLighting {
            sun: None,
            ..FrameLighting::default()
        })
        .mesh(
            MeshDraw::new(floor, Mat4::IDENTITY, unlit([0.5, 0.5, 0.5, 1.0])).with_lit(
                LitSurface {
                    fog: false,
                    ..LitSurface::default()
                },
            ),
        );
    } else {
        pass.mesh(MeshDraw::new(
            floor,
            Mat4::IDENTITY,
            unlit([0.5, 0.5, 0.5, 1.0]),
        ));
    }
    if wall {
        pass.mesh(
            MeshDraw::new(
                floor,
                Mat4::from_translation(Vec3::Z),
                unlit([0.0, 0.0, 1.0, 1.0]),
            )
            .with_lit(LitSurface {
                fog: false,
                ..LitSurface::default()
            }),
        );
    }
    for d in decals {
        pass.decal(d);
    }
    let stats = r.end_frame(frame)?;
    r.destroy_mesh(floor);
    Ok((
        r.read_pixels(&target)?.pixel(8, 8).unwrap_or_default(),
        stats,
    ))
}

fn near(p: u8, v: f32) -> bool {
    (i32::from(p) - (v * 255.0).round() as i32).abs() <= 3
}

#[test]
fn blend_colour_product_depth_and_counters() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let red_half = || square(0.0, 0.0, 1.5, [1.0, 0.0, 0.0, 0.5]);
    // Alpha blend over the grey receiver: 0.5 red + 0.5 grey.
    let (p, stats) = centre(&mut r, vec![DecalDraw::new(red_half())], false)?;
    assert!(
        near(p[0], 0.75) && near(p[1], 0.25) && near(p[2], 0.25),
        "{p:?}"
    );
    assert_eq!(
        (stats.decal_draws, stats.decal_triangles),
        (1, 2),
        "{stats:?}"
    );
    // Texture × vertex colour × material colour: a white texel, material (1, 1, 0.5, 1).
    let white = r.upload_texture(&TextureUpload {
        color_space: ColorSpace::Linear,
        ..TextureUpload::rgba8(1, 1, &[255, 255, 255, 255])
    })?;
    let d = DecalDraw {
        texture: Some(white),
        material: [1.0, 1.0, 0.5, 1.0],
        ..DecalDraw::new(square(0.0, 0.0, 1.5, [0.0, 1.0, 1.0, 1.0]))
    };
    let (p, _) = centre(&mut r, vec![d.clone()], false)?;
    assert!(
        near(p[0], 0.0) && near(p[1], 1.0) && near(p[2], 0.5),
        "{p:?}"
    );
    // A surface without a colour input (§6.1): the vertex colour has no effect.
    let (p, _) = centre(
        &mut r,
        vec![DecalDraw {
            vertex_colour: false,
            ..d
        }],
        false,
    )?;
    assert!(
        near(p[0], 1.0) && near(p[1], 1.0) && near(p[2], 0.5),
        "{p:?}"
    );
    // The depth test: a surface nearer the camera hides the decal.
    let (p, _) = centre(&mut r, vec![DecalDraw::new(red_half())], true)?;
    assert!(p[2] > 250 && p[0] < 5, "{p:?}");
    // Two batches: two draw calls.
    let (_, stats) = centre(
        &mut r,
        vec![DecalDraw::new(red_half()), DecalDraw::new(red_half())],
        false,
    )?;
    assert_eq!(stats.decal_draws, 2, "{stats:?}");
    // A batch without a whole triangle draws nothing.
    let (p, stats) = centre(
        &mut r,
        vec![DecalDraw::new(red_half().into_iter().take(2).collect())],
        false,
    )?;
    assert_eq!(stats.decal_draws, 0, "{stats:?}");
    assert!(near(p[0], 0.5), "{p:?}");
    Ok(())
}

/// A pass without frame lighting still draws its decals (they are unlit; `decals.md` §9.2
/// draws them with *L* = 1), unfogged, over the receiver.
#[test]
fn unlit_passes_draw_decals() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let (p, stats) = centre_in(
        &mut r,
        vec![DecalDraw::new(square(0.0, 0.0, 1.5, [1.0, 0.0, 0.0, 0.5]))],
        false,
        false,
    )?;
    assert_eq!(
        (stats.decal_draws, stats.decal_triangles),
        (1, 2),
        "{stats:?}"
    );
    assert!(
        near(p[0], 0.75) && near(p[1], 0.25) && near(p[2], 0.25),
        "{p:?}"
    );
    Ok(())
}

/// More batches than `MAX_DECAL_DRAWS_PER_PASS` never fail the frame: the batches beyond
/// the bound are left out with a warning.
#[test]
fn too_many_decal_batches_are_skipped_not_fatal() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let n = tessera_render::MAX_DECAL_DRAWS_PER_PASS + 3;
    let tri: Vec<DecalVertex> = square(0.0, 0.0, 0.01, [1.0, 0.0, 0.0, 1.0])
        .into_iter()
        .take(3)
        .collect();
    let draws: Vec<DecalDraw> = (0..n).map(|_| DecalDraw::new(tri.clone())).collect();
    let (_, stats) = centre(&mut r, draws, false)?;
    assert_eq!(
        stats.decal_draws as usize,
        tessera_render::MAX_DECAL_DRAWS_PER_PASS,
        "{stats:?}"
    );
    Ok(())
}
