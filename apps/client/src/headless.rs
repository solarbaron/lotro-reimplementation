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

/// How a `--screenshot` run waits for the spawn area to settle (`walker.md` §7). Other
/// headless runs keep their own waits.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SettleWait {
    /// At most `--settle-timeout` seconds of wall-clock time after the viewer is placed,
    /// and `--settle-timeout` + 120 s for the whole run (the command line).
    #[default]
    Timeout,
    /// As long as loading progresses, counted in frames rather than seconds, so a slow or
    /// loaded machine waits longer instead of giving up: the run gives up (screenshot
    /// taken, not settled) after `stall_frames` consecutive frames in which
    /// [`Walker::load_progress`](crate::walker::Walker::load_progress) did not change, or
    /// after `max_frames` loop iterations in all (start-up included). Frames are paced at
    /// 60 per second unless `--unpaced`, so both are at least that many 1/60 s of
    /// wall-clock time. For tests that must not depend on the machine's load.
    Progress {
        /// Consecutive frames without loading progress before giving up.
        stall_frames: u64,
        /// Loop iterations (start-up and frames) before giving up.
        max_frames: u64,
    },
}

/// What a headless run produced.
#[derive(Debug, Clone, Default)]
pub struct HeadlessReport {
    /// Frames rendered.
    pub frames: u64,
    /// Whether the area had settled when the screenshot was taken.
    pub settled: bool,
    /// What kept the area from settling when the screenshot was taken (empty when
    /// settled; `Walker::unsettled`).
    pub unsettled: Vec<&'static str>,
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

/// Runs the walker offscreen (module docs); a screenshot waits at most
/// `--settle-timeout` ([`SettleWait::Timeout`]).
///
/// # Errors
///
/// No install, no GPU, the walker could not start, or the output could not be written.
pub fn run(args: &Args) -> anyhow::Result<HeadlessReport> {
    run_with(args, SettleWait::Timeout)
}

/// Runs the walker offscreen (module docs); a screenshot waits for the area to settle as
/// `wait` says.
///
/// # Errors
///
/// No install, no GPU, the walker could not start, or the output could not be written.
pub fn run_with(args: &Args, wait: SettleWait) -> anyhow::Result<HeadlessReport> {
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
    // Progress waits replace the wall-clock limits of screenshot runs by frame counts.
    let progress_wait = match wait {
        SettleWait::Progress {
            stall_frames,
            max_frames,
        } if args.screenshot.is_some() && args.fly_path.is_none() && args.duration.is_none() => {
            Some((stall_frames, max_frames))
        }
        _ => None,
    };
    let mut report = HeadlessReport::default();
    let mut last = Instant::now();
    let mut settled_frames = 0u32;
    let mut viewport_set = false;
    let mut iterations = 0u64;
    let mut stall = StallCounter::default();
    let result = loop {
        iterations = iterations.saturating_add(1);
        let over_limit = |now: Instant| match progress_wait {
            Some((_, max_frames)) => iterations > max_frames,
            None => now > deadline,
        };
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
            if over_limit(now) {
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
            &mut rt.plugins,
            now,
            dt,
        );
        // A renderer error ends the loop through `finish`, which still shuts the walker
        // down and writes what it can.
        let drawn = (|| -> anyhow::Result<()> {
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
            Ok(())
        })();
        if let Err(e) = drawn {
            break Err(e);
        }
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
            let timed_out = match progress_wait {
                None => {
                    walker.placed()
                        && now.saturating_duration_since(started)
                            > Duration::from_secs(args.settle_timeout)
                }
                Some((stall_frames, max_frames)) => {
                    stall.observe(walker.load_progress()) >= stall_frames
                        || iterations >= max_frames
                }
            };
            if settled_frames >= SETTLED_FRAMES || timed_out {
                report.settled = settled_frames >= SETTLED_FRAMES;
                if !report.settled {
                    report.unsettled = walker.unsettled();
                    tracing::warn!(
                        waiting = ?report.unsettled,
                        stalled_frames = stall.stalled,
                        "area not fully loaded; taking the screenshot anyway"
                    );
                }
                break Ok(());
            }
        }
        if over_limit(now) {
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

/// Counts consecutive frames whose progress snapshot equals the previous one.
#[derive(Debug)]
struct StallCounter<P> {
    last: Option<P>,
    stalled: u64,
}

impl<P> Default for StallCounter<P> {
    fn default() -> Self {
        Self {
            last: None,
            stalled: 0,
        }
    }
}

impl<P: PartialEq> StallCounter<P> {
    /// Records this frame's snapshot; returns the frames in a row without a change.
    fn observe(&mut self, p: P) -> u64 {
        if self.last.as_ref() == Some(&p) {
            self.stalled = self.stalled.saturating_add(1);
        } else {
            self.last = Some(p);
            self.stalled = 0;
        }
        self.stalled
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stall_counter_resets_on_any_change() {
        let mut c = StallCounter::default();
        assert_eq!(c.observe(1), 0, "the first snapshot is progress");
        assert_eq!(c.observe(1), 1);
        assert_eq!(c.observe(1), 2);
        assert_eq!(c.observe(2), 0, "a change is progress");
        assert_eq!(c.observe(1), 0, "also a change back");
        assert_eq!(c.observe(1), 1);
    }

    #[test]
    fn a_new_pose_counts_as_progress() {
        use crate::walker::LoadProgress;
        let a = LoadProgress::default();
        let mut b = a;
        b.characters[2] += 1;
        let mut c = StallCounter::default();
        c.observe(a);
        assert_eq!(c.observe(a), 1);
        assert_eq!(c.observe(b), 0, "a new pose is progress");
    }

    #[test]
    fn settle_wait_defaults_to_the_command_line_timeout() {
        assert_eq!(SettleWait::default(), SettleWait::Timeout);
    }
}
