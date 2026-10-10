// SPDX-License-Identifier: GPL-3.0-or-later
//! The depth bias of a placed entity's draws (`docs/specs/decals.md` §6.5, `water.md` §14):
//! the Tessera default of §6.5 OPEN (c) applies the entity's `Render_DepthBias_Type` entry
//! to **every** colour draw of the entity, generic-path meshes (M2 and lit pipelines) and
//! instanced batches as well as own-program liquids, with the entry's slope-scale value as
//! the rasteriser's slope bias and its maximum as the bias clamp (W51), negated for the
//! reversed depth buffer (§6.5 Tessera Decision (sign)).
//!
//! Each check draws a red plane tilted along x and a green copy 0.01 m below it, seen from
//! above: without a bias the green one is hidden; a bias toward the camera (a negative
//! stored slope-scale value, `BiasForward*`) larger than the gap shows it; a positive one or
//! a clamp smaller than the gap leaves it hidden.

#[path = "support/gpu.rs"]
mod gpu;

use tessera_render::glam::Mat4;
use tessera_render::{
    AlphaMode, DrawDepthBias, FrameLighting, InstanceBatch, LitSurface, Material, MeshData,
    MeshDraw, OffscreenFormat, PassSpace, RenderError, Renderer, WorldSurface, wgpu,
};

const CLEAR: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
const GREEN: [f32; 4] = [0.0, 1.0, 0.0, 1.0];
/// The green plane lies this far below the red one (1e-4 of the depth range).
const GAP: f32 = 0.01;

/// A 4 × 4 plane over x, y in 0..4 rising one metre per metre along x, at height `z` at
/// x = 0.
fn tilted(z: f32) -> MeshData {
    let mut m = MeshData::new(
        vec![
            [0.0, 0.0, z],
            [4.0, 0.0, z + 4.0],
            [4.0, 4.0, z + 4.0],
            [0.0, 4.0, z],
        ],
        vec![0, 1, 2, 0, 2, 3],
    );
    let n = std::f32::consts::FRAC_1_SQRT_2;
    m.normals = Some(vec![[-n, 0.0, n]; 4]);
    m
}

/// Orthographic top-down view of x, y in 0..4, z in −50..50 (reversed depth).
fn top_down() -> Mat4 {
    let proj = tessera_render::glam::camera::rh::proj::directx::orthographic(
        0.0, 4.0, 0.0, 4.0, -50.0, 50.0,
    );
    Mat4::from_cols_array(&[
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 1.0, 1.0,
    ]) * proj
}

fn unlit(color: [f32; 4]) -> Material {
    Material {
        unlit: true,
        alpha: AlphaMode::Opaque,
        ..Material::color(color)
    }
}

fn clamp_supported(r: &Renderer<'_>) -> bool {
    r.context()
        .adapter
        .get_downlevel_capabilities()
        .flags
        .contains(wgpu::DownlevelFlags::DEPTH_BIAS_CLAMP)
}

/// The biases checked and whether the green plane shows with each.
fn cases(clamp: bool) -> [(DrawDepthBias, bool); 5] {
    [
        (DrawDepthBias::NONE, false),
        // `BiasForward*`: toward the camera, 2 × the depth slope ≫ the gap.
        (DrawDepthBias::slope(-2.0), true),
        // `BiasBackward*`: away from the camera.
        (DrawDepthBias::slope(2.0), false),
        // The maximum clamps the bias below the gap (when the adapter can clamp).
        (DrawDepthBias::new(-2.0, -1e-6), !clamp),
        // A clamp above the bias changes nothing.
        (DrawDepthBias::new(-2.0, -0.5), true),
    ]
}

/// Draws the red plane and, through `green`, the green one in a world pass (lit when `lit`)
/// and returns the centre pixel.
fn centre(
    r: &mut Renderer<'static>,
    lit: bool,
    green: impl FnOnce(&mut tessera_render::Pass),
) -> Result<[u8; 4], RenderError> {
    let red = r.upload_mesh(&tilted(0.0))?;
    let target = r.create_offscreen_target(16, 16, OffscreenFormat::Rgba8Unorm)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    let pass = frame.pass(
        "depth-bias",
        PassSpace::World {
            view_proj: top_down(),
        },
    );
    pass.clear(CLEAR);
    if lit {
        pass.frame_lighting(FrameLighting {
            sun: None,
            ..FrameLighting::default()
        });
    }
    pass.mesh(
        MeshDraw::new(red, Mat4::IDENTITY, unlit(RED)).with_lit(LitSurface {
            fog: false,
            ..LitSurface::default()
        }),
    );
    green(pass);
    r.end_frame(frame)?;
    r.destroy_mesh(red);
    Ok(r.read_pixels(&target)?.pixel(8, 8).unwrap_or_default())
}

fn is_green(px: [u8; 4]) -> bool {
    px[1] > 200 && px[0] < 50
}

fn is_red(px: [u8; 4]) -> bool {
    px[0] > 200 && px[1] < 50
}

/// `decals.md` §6.5 OPEN (c), Tessera default: a generic-path mesh draw of the entity takes
/// its bias, in the M2 (unlit pass) and the lit pipelines.
#[test]
fn generic_mesh_draws_apply_the_entity_bias() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let clamp = clamp_supported(&r);
    let green = r.upload_mesh(&tilted(-GAP))?;
    for lit in [false, true] {
        for (bias, shows) in cases(clamp) {
            let px = centre(&mut r, lit, |p| {
                p.mesh(
                    MeshDraw::new(green, Mat4::IDENTITY, unlit(GREEN))
                        .with_lit(LitSurface {
                            fog: false,
                            ..LitSurface::default()
                        })
                        .with_depth_bias(bias),
                );
            })?;
            let ok = if shows { is_green(px) } else { is_red(px) };
            assert!(ok, "lit {lit}, {bias:?}: {px:?}, green expected {shows}");
        }
    }
    r.destroy_mesh(green);
    Ok(())
}

/// The same for an instanced batch of the entity's parts (`InstanceBatch::depth_bias`).
#[test]
fn instanced_batches_apply_the_entity_bias() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let clamp = clamp_supported(&r);
    let model = r.upload_world_model(
        &tilted(-GAP),
        &[WorldSurface {
            base_color: GREEN,
            unlit: true,
            ..WorldSurface::default()
        }],
    )?;
    for (bias, shows) in cases(clamp) {
        let px = centre(&mut r, true, |p| {
            let mut batch = InstanceBatch::new(
                model.clone(),
                vec![tessera_render::Instance::new(Mat4::IDENTITY)],
            );
            batch.depth_bias = bias;
            p.instanced(batch);
        })?;
        let ok = if shows { is_green(px) } else { is_red(px) };
        assert!(ok, "{bias:?}: {px:?}, green expected {shows}");
    }
    Ok(())
}

/// Hostile biases draw like no bias (non-finite) or like the capped bias (huge), and never
/// fail the frame.
#[test]
fn hostile_biases_are_bounded() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let green = r.upload_mesh(&tilted(-GAP))?;
    for (bias, shows) in [
        (DrawDepthBias::slope(f32::NAN), false),
        (DrawDepthBias::new(f32::INFINITY, f32::NAN), false),
        (DrawDepthBias::slope(-1e30), true),
    ] {
        let px = centre(&mut r, true, |p| {
            p.mesh(
                MeshDraw::new(green, Mat4::IDENTITY, unlit(GREEN))
                    .with_lit(LitSurface {
                        fog: false,
                        ..LitSurface::default()
                    })
                    .with_depth_bias(bias),
            );
        })?;
        let ok = if shows { is_green(px) } else { is_red(px) };
        assert!(ok, "{bias:?}: {px:?}, green expected {shows}");
    }
    r.destroy_mesh(green);
    Ok(())
}
