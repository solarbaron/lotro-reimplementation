// SPDX-License-Identifier: GPL-3.0-or-later
//! Indicative cost of the UI pass at 3840 × 2160 (decision 0015 §6, U2; `render.md` §6.16).
//!
//! Not a gate: the device has no timestamp queries here (`UiStats::gpu_ns` is `None`), so
//! this measures wall-clock time per frame with a full GPU wait, with and without the UI
//! pass, and reports the difference as an upper bound of the UI pass's GPU time, plus the
//! CPU time of `end_frame`. Results are indicative and must be measured again on an idle
//! reference machine (`docs/perf/reference-machine.md`, decision 0010).
//!
//! ```sh
//! cargo test --release -p tessera-render --test ui_perf -- --ignored --nocapture
//! ```

#[path = "support/gpu.rs"]
mod gpu;
#[path = "ui_common/mod.rs"]
mod ui_common;

use std::time::{Duration, Instant};

use tessera_render::{
    OffscreenFormat, PassSpace, RenderError, UiAddressing, UiClip, UiEntry, UiRect, UiRotation,
    wgpu,
};
use ui_common::{solid, ui_texture};

const W: u32 = 3840;
const H: u32 = 2160;
const FRAMES: u32 = 60;

#[test]
#[ignore = "indicative measurement; run with --release --ignored --nocapture"]
fn ui_pass_cost_at_4k() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0; 4]) else {
        return Ok(());
    };
    // The scripted run's surfaces, roughly: a screen-sized plugin window with a full-size
    // background, P10 (1038 x 802), P7, the HUD set, a rotated window with a stretched
    // child, and the radar's nested entries.
    let screen = ui_texture(&mut r, W, H, |x, y| {
        [(x % 251) as u8, (y % 241) as u8, 80, 200]
    })?;
    let p10 = solid(&mut r, 1038, 802, [60, 60, 70, 230])?;
    let p7 = solid(&mut r, 577, 534, [70, 60, 50, 240])?;
    let small = solid(&mut r, 224, 280, [90, 90, 90, 180])?;
    let child = solid(&mut r, 128, 64, [200, 100, 50, 255])?;
    let tile = solid(&mut r, 32, 32, [40, 40, 40, 255])?;
    let target = r.create_offscreen_target(W, H, OffscreenFormat::Rgba8Srgb)?;
    let device = r.context().device.clone();
    let wait = || {
        device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(Duration::from_secs(10)),
        })
    };
    let run = |with_ui: bool, r: &mut tessera_render::Renderer<'static>| {
        let rot = [UiRotation::z([2000.0, 1000.0], 90.0)];
        let child_rot = [UiRotation::default(), rot[0]];
        let clip = [UiClip::new(UiRect::new(1800, 800, 400, 400), 1)];
        let mut cpu = Duration::ZERO;
        let start = Instant::now();
        for _ in 0..FRAMES {
            let mut frame = r.begin_offscreen_frame(&target)?;
            frame
                .pass("world", PassSpace::Screen)
                .clear([0.1, 0.2, 0.3, 1.0]);
            if with_ui {
                let p = frame.ui_pass();
                p.entry(&UiEntry::new(screen, [W, H], UiRect::new(0, 0, W, H)))
                    .entry(&UiEntry::new(
                        p10,
                        [1038, 802],
                        UiRect::new(1400, 600, 1038, 802),
                    ))
                    .entry(&UiEntry::new(
                        p7,
                        [577, 534],
                        UiRect::new(200, 300, 577, 534),
                    ))
                    .entry(
                        &UiEntry::new(p7, [577, 534], UiRect::new(1800, 800, 400, 400))
                            .with_rotations(&rot),
                    )
                    .entry(
                        &UiEntry::new(child, [128, 64], UiRect::new(1900, 900, 256, 128))
                            .with_rotations(&child_rot)
                            .with_clips(&clip),
                    )
                    .entry(
                        &UiEntry::new(tile, [32, 32], UiRect::new(0, 2000, W, 160))
                            .with_addressing(UiAddressing::Tile {
                                node_scale: [1.0, 1.0],
                            }),
                    );
                for i in 0..40 {
                    p.entry(&UiEntry::new(
                        small,
                        [224, 280],
                        UiRect::new(3000 + (i % 3) * 230, 100 + (i / 3) * 140, 224, 280),
                    ));
                }
            }
            let t = Instant::now();
            r.end_frame(frame)?;
            cpu += t.elapsed();
            wait()?;
        }
        Ok::<_, RenderError>((start.elapsed() / FRAMES, cpu / FRAMES))
    };
    // Warm up, then measure.
    run(true, &mut r)?;
    let (base, base_cpu) = run(false, &mut r)?;
    let (ui, ui_cpu) = run(true, &mut r)?;
    let info = r.adapter_info();
    eprintln!(
        "adapter {:?} ({:?}); {W}x{H}, {} entries: frame without UI {:.3} ms (end_frame CPU {:.3} ms), \
         with UI {:.3} ms (end_frame CPU {:.3} ms); UI pass upper bound {:.3} ms",
        info.name,
        info.backend,
        46,
        base.as_secs_f64() * 1e3,
        base_cpu.as_secs_f64() * 1e3,
        ui.as_secs_f64() * 1e3,
        ui_cpu.as_secs_f64() * 1e3,
        (ui.saturating_sub(base)).as_secs_f64() * 1e3,
    );
    Ok(())
}
