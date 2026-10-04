// SPDX-License-Identifier: GPL-3.0-or-later
//! `--smoke`: initialise every subsystem, render frames offscreen (no window) and exit.

use std::io::Write as _;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context as _, bail};
use tessera_render::{OffscreenFormat, Renderer, RendererConfig, RgbaImage};

use crate::cli::Args;
use crate::runtime::{DataStatus, Runtime};
use crate::scene::{CLEAR_COLOR, Scene};

/// Simulated frame time used by the smoke run.
pub const SMOKE_DT: Duration = Duration::from_millis(16);

/// What a smoke run produced.
#[derive(Debug, Clone)]
pub struct SmokeReport {
    /// Frames rendered.
    pub frames: u32,
    /// GPU adapter used.
    pub adapter: String,
    /// Surface DID on the quad, `None` for the procedural texture.
    pub surface: Option<u32>,
    /// Pixel at the image centre (on the quad).
    pub center: [u8; 4],
    /// Whether the centre differs from the clear colour (the quad and its opaque backing
    /// panel cover it).
    pub quad_visible: bool,
    /// Events the world session processed.
    pub net_events: u64,
    /// Plugins loaded at the end.
    pub plugins_loaded: usize,
    /// Data status at the end.
    pub data: DataStatus,
}

/// Run the smoke test.
pub fn run(args: &Args) -> anyhow::Result<SmokeReport> {
    let started = Instant::now();
    let mut clock = Instant::now();
    let mut rt = Runtime::start(args, clock)?;

    let mut renderer = Renderer::new_headless(&RendererConfig {
        clear_color: CLEAR_COLOR,
        ..RendererConfig::default()
    })
    .context("creating headless renderer")?;
    let info = renderer.adapter_info().clone();
    tracing::info!(adapter = %info.name, backend = ?info.backend, "headless renderer ready");
    let (w, h) = args.size;
    let target = renderer.create_offscreen_target(w, h, OffscreenFormat::Rgba8Srgb)?;
    let mut scene = Scene::new(&mut renderer)?;

    // Wait (bounded) for the showcase texture so the run exercises the real-data path.
    let deadline =
        crate::runtime::deadline_after(Instant::now(), Duration::from_secs(args.data_timeout));
    while !rt.texture_settled() {
        let now = Instant::now();
        if now >= deadline {
            tracing::warn!(
                timeout_s = args.data_timeout,
                "data loader still busy; rendering without it"
            );
            break;
        }
        if let Some(tex) = rt.wait_loader((deadline - now).min(Duration::from_millis(100))) {
            upload(&mut scene, &mut renderer, &tex);
        }
    }

    let mut last = None;
    for i in 0..args.frames {
        if let Some(tex) = rt.poll_loader() {
            upload(&mut scene, &mut renderer, &tex);
        }
        rt.tick(clock);
        scene.camera.update(SMOKE_DT.as_secs_f32());
        let mut frame = renderer.begin_offscreen_frame(&target)?;
        rt.draw(&scene, &mut frame);
        let stats = renderer.end_frame(frame)?;
        tracing::debug!(frame = i, ?stats, "smoke frame");
        last = Some(stats);
        clock += SMOKE_DT;
    }
    let image = renderer.read_pixels(&target)?;
    let center = image.pixel(w / 2, h / 2).unwrap_or_default();
    let clear_srgb = CLEAR_COLOR.map(linear_to_srgb8);
    let quad_visible = center[..3] != clear_srgb[..3];
    if let Some(path) = &args.smoke_out {
        crate::runtime::refuse_install_path(args, Some(&rt.status), path)?;
        write_ppm(path, &image).with_context(|| format!("writing {}", path.display()))?;
        tracing::info!(path = %path.display(), "smoke image written");
    }

    // Let the loader finish its statistics pass before shutting down (bounded).
    while !rt.loader_finished() && Instant::now() < deadline {
        let _ = rt.wait_loader(Duration::from_millis(100));
    }
    let report = SmokeReport {
        frames: args.frames,
        adapter: info.name.clone(),
        surface: scene.shown().did,
        center,
        quad_visible,
        net_events: rt.world.as_ref().map_or(0, crate::net::World::events_seen),
        plugins_loaded: rt.plugins.manager().loaded_plugins().len(),
        data: rt.status.clone(),
    };
    rt.shutdown();
    tracing::info!(
        frames = report.frames,
        last = ?last,
        surface = %report.surface.map_or_else(|| "procedural".to_owned(), |d| format!("{d:#010x}")),
        center = ?report.center,
        quad_visible = report.quad_visible,
        net_events = report.net_events,
        plugins = report.plugins_loaded,
        data = ?report.data,
        elapsed_ms = started.elapsed().as_millis(),
        "smoke run complete"
    );
    if !report.quad_visible {
        bail!("smoke: the showcase quad is not visible at the image centre");
    }
    Ok(report)
}

fn upload(scene: &mut Scene, renderer: &mut Renderer<'_>, tex: &crate::data::DecodedTexture) {
    if let Err(e) = scene.set_surface(renderer, tex) {
        tracing::warn!(
            did = format_args!("{:#010x}", tex.did),
            "texture upload failed: {e}"
        );
    }
}

/// Linear float to 8-bit sRGB.
#[must_use]
pub fn linear_to_srgb8(v: f32) -> u8 {
    let v = if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let s = if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0).round().clamp(0.0, 255.0) as u8
}

/// Binary PPM (P6), alpha dropped. Our own output; never written into the install.
pub fn write_ppm(path: &Path, image: &RgbaImage) -> std::io::Result<()> {
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
    write!(f, "P6\n{} {}\n255\n", image.width, image.height)?;
    for px in image.data.as_chunks::<4>().0 {
        f.write_all(&px[..3])?;
    }
    f.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srgb_conversion() {
        assert_eq!(linear_to_srgb8(0.0), 0);
        assert_eq!(linear_to_srgb8(1.0), 255);
        assert_eq!(linear_to_srgb8(0.5), 188);
        assert_eq!(linear_to_srgb8(f32::NAN), 0);
    }
}
