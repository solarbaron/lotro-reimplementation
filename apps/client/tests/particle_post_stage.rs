// SPDX-License-Identifier: GPL-3.0-or-later
//! `particle-runtime.md` §12 rule 16 in the walker: the systems of the pass after
//! post-processing are queued in the last stage of the frame's world pass
//! (`Pass::post_particles`), not on a world pass of their own; only a frame without a world
//! pass gets one. Synthetic draws, no game data; skips without a GPU adapter.

use tessera_client::walker::effects::post_stage;
use tessera_render::{
    Camera, ContextOptions, OffscreenFormat, ParticleDraw, PassSpace, Renderer, RendererConfig,
};

fn renderer() -> Option<Renderer<'static>> {
    let config = RendererConfig {
        context: ContextOptions {
            allow_software: true,
            ..ContextOptions::default()
        },
        ..RendererConfig::default()
    };
    match Renderer::new_headless(&config) {
        Ok(r) => Some(r),
        Err(e) => {
            eprintln!("SKIPPED: no GPU renderer: {e}");
            None
        }
    }
}

#[test]
fn post_draws_go_to_the_world_pass_last_stage() {
    let Some(mut r) = renderer() else {
        return;
    };
    let Ok(target) = r.create_offscreen_target(8, 8, OffscreenFormat::Rgba8Unorm) else {
        eprintln!("SKIPPED: no offscreen target");
        return;
    };
    let camera = Camera::default();
    let draws = || vec![ParticleDraw::new(Vec::new(), 0); 3];

    let mut frame = r.begin_offscreen_frame(&target).expect("frame");
    frame.world_pass("walker-world", &camera);
    frame.pass("walker-hud", PassSpace::Screen);
    post_stage(&mut frame, &camera, None, draws());
    assert_eq!(frame.passes().len(), 2, "no pass appended");
    assert_eq!(frame.passes()[0].post_particle_count(), 3);
    assert_eq!(frame.passes()[1].post_particle_count(), 0);
    // No draws: nothing queued.
    post_stage(&mut frame, &camera, None, Vec::new());
    assert_eq!(frame.passes()[0].post_particle_count(), 3);
    r.end_frame(frame).expect("end frame");

    // Without a world pass the draws get one of their own.
    let mut frame = r.begin_offscreen_frame(&target).expect("frame");
    frame.pass("screen", PassSpace::Screen);
    post_stage(&mut frame, &camera, None, draws());
    assert_eq!(frame.passes().len(), 2);
    assert!(matches!(frame.passes()[1].space(), PassSpace::World { .. }));
    assert_eq!(frame.passes()[1].label(), "walker-world-post");
    r.end_frame(frame).expect("end frame");
}
