// SPDX-License-Identifier: GPL-3.0-or-later
//! CPU cost of skinned drawing: 50 characters of 30 skinned parts each (1,500 draws), one
//! 89-bone palette per character and frame, compared with the same draws on the static
//! path. Run with `cargo bench -p tessera-render --bench skinned` (add `-- --frames N` to
//! change the frame count). Prints per-frame CPU times; GPU time is excluded by waiting
//! for the device after each frame, outside the timed region.
//!
//! Synthetic meshes only. Skips (exit 0) when no adapter is available.

#[path = "../tests/support/skinning.rs"]
mod skinning;

use std::time::{Duration, Instant};

use tessera_render::glam::{Mat4, Vec3};
use tessera_render::{
    Camera, ContextOptions, Material, MeshDraw, MeshId, MeshSkin, OffscreenFormat, RenderError,
    Renderer, RendererConfig, wgpu,
};

const CHARACTERS: usize = 50;
const PARTS: usize = 30;
const BONES: usize = 89;

struct Timing {
    build: Duration,
    end_frame: Duration,
}

fn renderer() -> Option<Renderer<'static>> {
    for backends in [None, Some(wgpu::Backends::VULKAN), Some(wgpu::Backends::GL)] {
        let config = RendererConfig {
            context: ContextOptions {
                backends,
                allow_software: true,
                ..ContextOptions::default()
            },
            ..RendererConfig::default()
        };
        if let Ok(r) = Renderer::new_headless(&config) {
            return Some(r);
        }
    }
    None
}

/// 30 distinct part meshes (about 200 vertices each), every part bound to two of the 89
/// bones.
fn upload_parts(r: &mut Renderer<'static>) -> Result<Vec<MeshId>, RenderError> {
    (0..PARTS)
        .map(|i| {
            let mesh = skinning::cylinder(0.06, 0.25, 12, 14);
            let mut skin: MeshSkin = skinning::two_bone_skin(&mesh, 0.125, 0.05, BONES as u16);
            let (a, b) = ((i * 3 % BONES) as u8, ((i * 3 + 1) % BONES) as u8);
            for j in &mut skin.joints {
                *j = j.map(|x| if x == 0 { a } else { b });
            }
            r.upload_skinned_mesh(&mesh, &skin)
        })
        .collect()
}

fn run(
    r: &mut Renderer<'static>,
    parts: &[MeshId],
    frames: usize,
    skinned: bool,
) -> Result<Timing, RenderError> {
    let target = r.create_offscreen_target(256, 256, OffscreenFormat::Rgba8Srgb)?;
    let cam = Camera {
        eye: Vec3::new(0.0, 4.0, 12.0),
        target: Vec3::ZERO,
        up: Vec3::Y,
        ..Camera::default()
    };
    let material = Material::color([0.7, 0.6, 0.5, 1.0]);
    let mut palette = vec![Mat4::IDENTITY; BONES];
    let mut timing = Timing {
        build: Duration::ZERO,
        end_frame: Duration::ZERO,
    };
    for frame_no in 0..frames {
        let start = Instant::now();
        let mut frame = r.begin_offscreen_frame(&target)?;
        let mut draws = Vec::with_capacity(CHARACTERS * PARTS);
        for c in 0..CHARACTERS {
            let place = Mat4::from_translation(Vec3::new(
                (c % 10) as f32 - 4.5,
                0.0,
                (c / 10) as f32 - 2.0,
            ));
            let skin = if skinned {
                // A cheap stand-in for a sampled pose: every bone turns a little.
                let t = (frame_no + c) as f32 * 0.05;
                for (b, m) in palette.iter_mut().enumerate() {
                    *m = Mat4::from_rotation_z((t + b as f32 * 0.1).sin() * 0.3);
                }
                Some(frame.skin_palette(&palette)?)
            } else {
                None
            };
            for (p, &mesh) in parts.iter().enumerate() {
                let at = place * Mat4::from_translation(Vec3::new(0.0, p as f32 * 0.05, 0.0));
                let draw = MeshDraw::new(mesh, at, material);
                draws.push(match skin {
                    Some(s) => draw.with_skin(s),
                    None => draw,
                });
            }
        }
        let pass = frame.world_pass("crowd", &cam);
        pass.clear([0.0, 0.0, 0.0, 1.0]);
        for d in draws {
            pass.mesh(d);
        }
        let built = Instant::now();
        let stats = r.end_frame(frame)?;
        let ended = Instant::now();
        assert_eq!(stats.mesh_draws as usize, CHARACTERS * PARTS);
        assert_eq!(
            stats.skinned_draws as usize,
            if skinned { CHARACTERS * PARTS } else { 0 }
        );
        // Untimed: let the GPU finish so frames do not queue up.
        let _ = r.context().device.poll(wgpu::PollType::wait_indefinitely());
        if frame_no > 0 {
            // Frame 0 builds pipelines and grows buffers.
            timing.build += built - start;
            timing.end_frame += ended - built;
        }
    }
    Ok(timing)
}

fn main() -> Result<(), RenderError> {
    let frames = std::env::args()
        .skip_while(|a| a != "--frames")
        .nth(1)
        .and_then(|n| n.parse().ok())
        .unwrap_or(60usize)
        .max(2);
    let Some(mut r) = renderer() else {
        println!("SKIPPED: no GPU adapter available");
        return Ok(());
    };
    let info = r.adapter_info().clone();
    println!(
        "adapter: {} ({:?}); {CHARACTERS} characters x {PARTS} parts, {BONES}-bone palettes, \
         {frames} frames",
        info.name, info.backend
    );
    let parts = upload_parts(&mut r)?;
    let per_frame = |d: Duration| d.as_secs_f64() * 1e3 / (frames - 1) as f64;
    for (label, skinned) in [("static", false), ("skinned", true)] {
        let t = run(&mut r, &parts, frames, skinned)?;
        println!(
            "{label:>8}: build {:.3} ms/frame, end_frame {:.3} ms/frame, total CPU {:.3} ms/frame",
            per_frame(t.build),
            per_frame(t.end_frame),
            per_frame(t.build + t.end_frame)
        );
    }
    Ok(())
}
