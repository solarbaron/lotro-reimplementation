// SPDX-License-Identifier: GPL-3.0-or-later
//! The particle stage after post-processing (`docs/specs/particle-runtime.md` §12 rule 16):
//! draws queued with `Pass::post_particles` come after every other draw of the world pass,
//! the sorted blended meshes and the ordinary particle draws (`Pass::particles`) included,
//! with the same depth test and drawing.
//!
//! Synthetic geometry only (no game data).

#[path = "support/gpu.rs"]
mod gpu;

use tessera_render::glam::{Mat4, Vec3};
use tessera_render::{
    AlphaMode, Camera, Material, MeshData, MeshDraw, OffscreenFormat, ParticleDraw, ParticleVertex,
    PassSpace, RenderError, Renderer,
};

const CLEAR: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

/// One quad covering the centre of a 16 × 16 target at depth `z`, vertex colour `color`,
/// alpha-blended.
fn quad(color: [f32; 4], z: f32) -> ParticleDraw {
    let v = |x: f32, y: f32| ParticleVertex {
        position: [x, y, z],
        color,
        uv: [0.5, 0.5],
    };
    ParticleDraw {
        alpha: AlphaMode::Blend,
        ..ParticleDraw::new(
            vec![v(-1.0, 1.0), v(-1.0, -1.0), v(1.0, -1.0), v(1.0, 1.0)],
            4,
        )
    }
}

fn camera() -> Camera {
    Camera {
        eye: Vec3::new(0.0, 0.0, 5.0),
        target: Vec3::ZERO,
        up: Vec3::Y,
        ..Camera::default()
    }
}

/// What a pass queues besides its grey clear.
#[derive(Clone, Copy)]
enum Queue {
    /// A blended blue plane behind (z = −0.5) and a red particle in front (z = 0), the
    /// particle as an ordinary draw.
    MeshThenParticle,
    /// The same with the particle in the post stage.
    MeshThenPost,
    /// A green ordinary particle and a red post particle, both at z = 0, the post one queued
    /// first.
    PostQueuedFirst,
    /// Only a red post particle behind an opaque blue wall nearer the camera.
    PostBehindWall,
}

/// Draws `queue` over a mid-grey background and returns the centre pixel and the particle
/// draw count.
fn centre(r: &mut Renderer<'static>, queue: Queue) -> Result<([u8; 4], u32), RenderError> {
    let target = r.create_offscreen_target(16, 16, OffscreenFormat::Rgba8Unorm)?;
    let plane = r.upload_mesh(&MeshData::plane(4.0, 4.0, 1, 1))?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    let pass = frame.world_pass("p", &camera());
    pass.clear([0.5, 0.5, 0.5, 1.0]);
    let blue = |alpha: AlphaMode, a: f32, z: f32| {
        MeshDraw::new(
            plane,
            Mat4::from_translation(Vec3::new(0.0, 0.0, z)),
            Material {
                unlit: true,
                alpha,
                ..Material::color([0.0, 0.0, 1.0, a])
            },
        )
    };
    let red = [1.0, 0.0, 0.0, 0.5];
    match queue {
        Queue::MeshThenParticle => {
            pass.mesh(blue(AlphaMode::Blend, 0.5, -0.5));
            pass.particles(quad(red, 0.0));
        }
        Queue::MeshThenPost => {
            pass.mesh(blue(AlphaMode::Blend, 0.5, -0.5));
            pass.post_particles(quad(red, 0.0));
        }
        Queue::PostQueuedFirst => {
            pass.post_particles(quad(red, 0.0));
            pass.particles(quad([0.0, 1.0, 0.0, 0.5], 0.0));
        }
        Queue::PostBehindWall => {
            pass.mesh(blue(AlphaMode::Opaque, 1.0, 1.0));
            pass.post_particles(quad(red, 0.0));
        }
    }
    assert!(pass.post_particle_count() <= 1);
    let stats = r.end_frame(frame)?;
    let px = r.read_pixels(&target)?.pixel(8, 8).unwrap_or_default();
    Ok((px, stats.particle_draws))
}

fn near(p: [u8; 4], rgb: [f32; 3]) -> bool {
    (0..3).all(|i| (i32::from(p[i]) - (rgb[i] * 255.0).round() as i32).abs() <= 3)
}

/// Rule 16: an ordinary particle draw comes before the sorted blended meshes (so the
/// blended plane behind it is drawn over it), a post-stage draw after them.
#[test]
fn post_stage_follows_the_blended_meshes() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    // Translucent stage, then the blended plane: blue 0.5 over (red 0.5 over grey).
    let (p, n) = centre(&mut r, Queue::MeshThenParticle)?;
    assert_eq!(n, 1);
    assert!(near(p, [0.375, 0.125, 0.625]), "{p:?}");
    // Post stage: red 0.5 over (blue 0.5 over grey).
    let (p, n) = centre(&mut r, Queue::MeshThenPost)?;
    assert_eq!(n, 1);
    assert!(near(p, [0.625, 0.125, 0.375]), "{p:?}");
    Ok(())
}

/// Rule 16: post-stage draws follow the pass's other particle draws whatever the order of
/// the calls, and keep the depth test.
#[test]
fn post_stage_follows_the_other_particles_with_the_depth_test() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    // Green 0.5 first, red 0.5 over it: (0.625, 0.375, 0.125); the other order gives
    // (0.375, 0.625, 0.125).
    let (p, n) = centre(&mut r, Queue::PostQueuedFirst)?;
    assert_eq!(n, 2);
    assert!(near(p, [0.625, 0.375, 0.125]), "{p:?}");
    // A nearer opaque wall hides the post-stage particle.
    let (p, _) = centre(&mut r, Queue::PostBehindWall)?;
    assert!(near(p, [0.0, 0.0, 1.0]), "{p:?}");
    Ok(())
}

/// Screen passes ignore post-stage draws, as they ignore ordinary particle draws.
#[test]
fn screen_passes_ignore_post_stage_draws() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let target = r.create_offscreen_target(16, 16, OffscreenFormat::Rgba8Unorm)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    frame
        .pass("screen", PassSpace::Screen)
        .clear([0.5, 0.5, 0.5, 1.0])
        .post_particles(quad([1.0, 0.0, 0.0, 1.0], 0.0));
    let stats = r.end_frame(frame)?;
    assert_eq!(stats.particle_draws, 0);
    let p = r.read_pixels(&target)?.pixel(8, 8).unwrap_or_default();
    assert!(near(p, [0.5, 0.5, 0.5]), "{p:?}");
    Ok(())
}
