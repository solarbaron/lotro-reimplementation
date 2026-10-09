// SPDX-License-Identifier: GPL-3.0-or-later
//! The world walker without a window: `--screenshot <png>` (render the spawn point once
//! everything around it has loaded), `--fly-path ... --headless` (a scripted flight with a
//! frame log), `--duration SECS --headless` (a timing run once the spawn has settled) and
//! `--walk-path FILE` (scripted walks on collision, without a GPU: `walker::movement::run`).
//! Frames are rendered offscreen at `--size`, paced at 60 per second (unless
//! `--unpaced`), and each frame waits for the GPU so frame times include GPU work.

use std::time::{Duration, Instant};

use anyhow::{Context as _, bail};
use tessera_render::{OffscreenFormat, Renderer, RendererConfig};

use crate::cli::Args;
use crate::runtime::{Runtime, deadline_after, refuse_install_path};
use crate::walker::SKY;
use crate::walker::framelog::Summary;
use crate::walker_host::{WalkerHost, capture, save_png};

/// Frame pacing of headless runs.
pub const FRAME: Duration = Duration::from_micros(16_667);
/// Consecutive settled frames before a screenshot is taken.
const SETTLED_FRAMES: u32 = 20;

/// What a headless run produced.
#[derive(Debug, Clone, Default)]
pub struct HeadlessReport {
    /// Frames rendered.
    pub frames: u64,
    /// Whether the area had settled when the screenshot was taken.
    pub settled: bool,
    /// The flight's frame summary.
    pub summary: Option<Summary>,
    /// Where the viewer ended.
    pub description: Option<String>,
    /// The entities placed by the landblocks (NPCs, weenie placements) when the run
    /// ended: sandbox tag, region-global position and heading (radians, 0 = east,
    /// counter-clockwise), as the character layer holds them.
    pub placed: Vec<PlacedReport>,
    /// Character counters at the end.
    pub characters: crate::walker::characters::CharacterStats,
    /// World audio at the end: counters and the music events (ROADMAP M6 criterion 2).
    pub audio: Option<crate::audio::AudioReport>,
    /// The far view's counters of the last frame (`--far-view on`).
    pub far_view: Option<crate::walker::farview::FarViewStats>,
}

/// One placed figure of a [`HeadlessReport`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacedReport {
    /// The sandbox tag (landblock-data DID and figure index).
    pub tag: u64,
    /// Region.
    pub region: u8,
    /// Region-global position.
    pub position: [f64; 3],
    /// Heading.
    pub heading: f32,
    /// The character layer drew the figure at the end of the run (figure on the GPU,
    /// posed, within the character distance; `walker.md` §10.2), or hides it as a marker.
    /// `false` while it is still a static instance: building, failed or too far.
    pub figure_shown: bool,
}

/// Runs the walker offscreen (module docs).
///
/// # Errors
///
/// No install, no GPU, the walker could not start, or the output could not be written.
pub fn run(args: &Args) -> anyhow::Result<HeadlessReport> {
    if args.walk_path.is_some() {
        // Scripted walks need no GPU (`walker::movement::run`).
        let report = crate::walker::movement::run::run_cli(args)?;
        if report.failed > 0 {
            bail!(
                "{} of {} scripted walks failed",
                report.failed,
                report.failed + report.passed
            );
        }
        return Ok(HeadlessReport::default());
    }
    if args.screenshot.is_none() && args.fly_path.is_none() && args.duration.is_none() {
        bail!("a headless walker run needs --screenshot, --fly-path or --duration to end");
    }
    // Fail before any work when an output would land in the install (checked again with
    // the discovered install before writing).
    for path in args.screenshot.iter().chain(&args.frame_log) {
        refuse_install_path(args, None, path)?;
    }
    let mut rt = Runtime::start(args, Instant::now())?;
    let shared = rt
        .walker_shared
        .clone()
        .context("walker session not configured")?;
    let mut renderer = Renderer::new_headless(&RendererConfig {
        clear_color: SKY,
        meshes: crate::walker::mesh_config(crate::walker::arena_bytes(
            &crate::walker_host::walker_config(args),
        )),
        ..RendererConfig::default()
    })
    .context("creating the headless renderer")?;
    let info = renderer.adapter_info().clone();
    tracing::info!(adapter = %info.name, backend = ?info.backend, "headless walker renderer ready");
    let (w, h) = args.size;
    let target = renderer.create_offscreen_target(w, h, OffscreenFormat::Rgba8Srgb)?;
    let mut host = WalkerHost::new(args, shared);
    let started = Instant::now();
    let flight_secs = args
        .fly_path
        .map_or(0.0, |p| 3.0 * p.length() / args.speed.max(1e-3))
        .min(1.0e6);
    let deadline = deadline_after(
        started,
        Duration::from_secs(args.settle_timeout.saturating_add(120))
            .saturating_add(Duration::from_secs_f64(flight_secs.max(0.0)))
            .saturating_add(Duration::from_secs(args.duration.unwrap_or(0))),
    );
    let mut report = HeadlessReport::default();
    let mut last = Instant::now();
    let mut settled_frames = 0u32;
    let mut viewport_set = false;
    let result = loop {
        let now = Instant::now();
        let dt = now.saturating_duration_since(last).as_secs_f32();
        last = now;
        let _ = rt.poll_loader();
        rt.tick(now);
        host.poll(&mut rt, &mut renderer);
        if let Some(reason) = &host.failed {
            break Err(anyhow::anyhow!("world walker failed: {reason}"));
        }
        let notices = rt.take_notices();
        let Some(walker) = host.walker.as_mut() else {
            if now > deadline {
                break Err(anyhow::anyhow!("world did not load in time"));
            }
            std::thread::sleep(Duration::from_millis(20));
            continue;
        };
        if !viewport_set {
            walker.set_viewport(w, h);
            viewport_set = true;
        }
        for n in notices {
            tracing::info!("world: {n}");
            walker.notice(n);
        }
        let ev = walker.update(
            &mut renderer,
            rt.world
                .as_mut()
                .map(|w| w as &mut dyn crate::walker::WalkerSession),
            now,
            dt,
        );
        let mut frame = renderer.begin_offscreen_frame(&target)?;
        walker.draw(&renderer, &mut frame);
        let t_end = Instant::now();
        let stats = renderer.end_frame(frame)?;
        walker.record_render_stats(&stats);
        let t_wait = Instant::now();
        renderer
            .context()
            .wait_idle()
            .context("waiting for the GPU")?;
        walker.record_frame_timing(
            t_wait.saturating_duration_since(t_end).as_secs_f64() * 1e3,
            t_wait.elapsed().as_secs_f64() * 1e3,
        );
        report.frames += 1;

        if args.fly_path.is_some() {
            if ev.flight_done {
                report.summary = walker.frame_log().map(|l| l.summary());
                break Ok(());
            }
        } else if args.duration.is_some() {
            if ev.duration_done {
                report.summary = walker.frame_log().map(|l| l.summary());
                report.settled = walker.settled();
                break Ok(());
            }
        } else if args.screenshot.is_some() {
            settled_frames = if walker.settled() {
                settled_frames + 1
            } else {
                0
            };
            let timed_out = walker.placed()
                && now.saturating_duration_since(started)
                    > Duration::from_secs(args.settle_timeout);
            if settled_frames >= SETTLED_FRAMES || timed_out {
                report.settled = settled_frames >= SETTLED_FRAMES;
                if !report.settled {
                    tracing::warn!(
                        waiting = ?walker.unsettled(),
                        "area not fully loaded; taking the screenshot anyway"
                    );
                }
                break Ok(());
            }
        }
        if now > deadline {
            break Err(anyhow::anyhow!("timed out"));
        }
        if ev.quit {
            break Ok(());
        }
        let spent = now.elapsed();
        if spent < FRAME && !args.unpaced {
            std::thread::sleep(FRAME - spent);
        }
    };
    let out = finish(args, &mut rt, &mut host, &mut renderer, &mut report, result);
    rt.shutdown();
    out.map(|()| report)
}

fn finish(
    args: &Args,
    rt: &mut Runtime,
    host: &mut WalkerHost,
    renderer: &mut Renderer<'_>,
    report: &mut HeadlessReport,
    result: anyhow::Result<()>,
) -> anyhow::Result<()> {
    let Some(walker) = host.walker.as_mut() else {
        return result;
    };
    // Every step runs even when an earlier one failed (the walker must always shut down);
    // the first error is returned.
    let mut first_error: Option<anyhow::Error> = result.err();
    report.description = walker.describe();
    let chars = walker.characters();
    report.characters = chars.stats();
    report.placed = chars
        .placed()
        .filter_map(|(id, tag)| {
            let (p, heading) = chars.entity_pose(id)?;
            Some(PlacedReport {
                tag,
                region: p.region,
                position: [p.x, p.y, p.z],
                heading,
                figure_shown: chars.shows_figure(tag),
            })
        })
        .collect();
    if let Some(d) = &report.description {
        tracing::info!("viewer at: {d}");
    }
    report.audio = walker.audio().map(crate::audio::WorldAudio::report);
    report.far_view = walker.far_view_stats();
    if let Some(f) = &report.far_view {
        tracing::info!(?f, "far view at the end");
    }
    if let Some(a) = &report.audio {
        crate::audio::log_report(a);
    }
    if first_error.is_none()
        && let Some(path) = &args.screenshot
    {
        let r = (|| {
            refuse_install_path(args, Some(&rt.status), path)?;
            let image = capture(renderer, walker, args.size)?;
            save_png(path, &image)?;
            tracing::info!(path = %path.display(), settled = report.settled, "screenshot saved");
            Ok(())
        })();
        keep(&mut first_error, r);
    }
    if let Some(log) = walker.frame_log() {
        if let Some(path) = &args.frame_log {
            let r = (|| {
                refuse_install_path(args, Some(&rt.status), path)?;
                log.write_csv(path)
                    .with_context(|| format!("writing {}", path.display()))?;
                tracing::info!(path = %path.display(), rows = log.rows().len(), "frame log written");
                Ok(())
            })();
            keep(&mut first_error, r);
        }
        let s = log.summary();
        tracing::info!("frame summary: {s}");
        report.summary.get_or_insert(s);
    }
    let st = walker.stream_stats();
    tracing::info!(
        requested = st.requested,
        integrated = st.integrated,
        evicted = st.evicted,
        failed = st.failed,
        peak_resident_mib = st.peak_resident_bytes >> 20,
        peak_blocks = st.peak_resident_blocks,
        max_update_ms = st.max_frame.as_secs_f64() * 1e3,
        "streamer totals"
    );
    walker.shutdown(renderer);
    match first_error {
        Some(e) => bail!("{e:#}"),
        None => Ok(()),
    }
}

/// Records `r`'s error unless an earlier one is already kept.
fn keep(first: &mut Option<anyhow::Error>, r: anyhow::Result<()>) {
    if let Err(e) = r
        && first.is_none()
    {
        *first = Some(e);
    }
}
