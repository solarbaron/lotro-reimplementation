// SPDX-License-Identifier: GPL-3.0-or-later
//! Headless walker runs with the game UI attached (`--headless` or `--screenshot` together
//! with `--ui`, `--open-panel`, `--skin` or `--ui-script`): the walker's offscreen loop
//! of [`crate::headless`] with the UI steps of [`super::client`], for checklist screenshots
//! (no cursor is ever drawn into them) and for the scripted measurement of decision 0015
//! §6. Outputs are refused inside the install.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context as _, bail};
use tessera_render::{OffscreenFormat, OffscreenTarget, Renderer, RendererConfig, RgbaImage};

use super::client::ClientUi;
use super::cursor::NoWindow;
use super::script::write_budget_plugin;
use super::stats::report_markdown;
use crate::cli::Args;
use crate::headless::FRAME;
use crate::runtime::{Runtime, data_root, deadline_after, refuse_install_path};
use crate::walker::{SKY, Walker};
use crate::walker_host::{WalkerHost, save_png};

/// Consecutive frames in which the world and the UI are settled before a screenshot.
const SETTLED_FRAMES: u32 = 30;
/// Frames of each half of the U2 proxy measurement.
const U2_FRAMES: u32 = 60;
/// Most UI steps the capture settle of a screenshot runs ([`capture_settle`]).
const CAPTURE_SETTLE_STEPS: u32 = 5_000;
/// Most wall time the capture settle of a screenshot runs ([`capture_settle`]).
const CAPTURE_SETTLE_LIMIT: Duration = Duration::from_secs(60);

/// What a headless UI run produced.
#[derive(Debug, Clone, Default)]
pub struct UiRunReport {
    /// Frames rendered.
    pub frames: u64,
    /// Whether the world and the UI had settled for the screenshot.
    pub settled: bool,
    /// The U1–U5 report (with `--ui-script` or `--ui-report`).
    pub report: Option<String>,
    /// Native panels open at the end.
    pub panels: usize,
    /// Plugins loaded at the end.
    pub plugins: Vec<String>,
    /// Short names of the game windows (`npc`, `quest`, `travel`, `finder`, and the P6
    /// `crafting`, `profession-trainer`, `vocation-trainer`, `craftguild-trainer`,
    /// `training`, `traits`, `experience`) seen open at any frame of the run: the per-frame
    /// window updates of `super::client::window_frames` drive them from the session's
    /// events (P3/P5/P6) and `--open-panel`, so a headless scenario run can check that they
    /// opened. The run logs them at its end (`game windows seen`).
    pub game_windows: std::collections::BTreeSet<&'static str>,
    /// The timing report of a `--scenario-timing` run ([`crate::timing`]).
    pub timing: Option<crate::timing::TimingReport>,
}

/// The arguments of a run: with `--ui-script` and no `--plugins`, the scripted run's own
/// Plugins folder (`<data dir>/ui-script/Plugins`) holding its synthetic plugin, which is
/// loaded. Other runs are unchanged.
///
/// # Errors
///
/// When the folder would be inside the install or cannot be written.
pub fn script_args(args: &Args) -> anyhow::Result<Args> {
    let mut a = args.clone();
    if args.ui_script {
        if a.plugins.is_none() {
            let dir: PathBuf = data_root(args).join("ui-script").join("Plugins");
            refuse_install_path(args, None, &dir)?;
            write_budget_plugin(&dir)
                .with_context(|| format!("writing the script plugin to {}", dir.display()))?;
            a.plugins = Some(dir);
        }
        if !a.load_plugins.iter().any(|p| p == "Budget") {
            a.load_plugins.push("Budget".into());
        }
    }
    Ok(a)
}

fn render_frame(
    renderer: &mut Renderer<'_>,
    target: &OffscreenTarget,
    walker: &mut Walker,
    ui: Option<&mut ClientUi>,
) -> anyhow::Result<()> {
    let mut ui = ui;
    if let Some(ui) = ui.as_deref_mut() {
        // The paperdoll's portrait, before the frame that composes it.
        ui.render_portraits(walker, renderer);
    }
    let mut frame = renderer.begin_offscreen_frame(target)?;
    walker.draw(renderer, &mut frame);
    if let Some(ui) = ui.as_deref_mut() {
        ui.draw(&mut frame, false);
    }
    let t_end = std::time::Instant::now();
    let stats = renderer.end_frame(frame)?;
    walker.record_render_stats(&stats);
    if let Some(ui) = ui {
        // U2 (render.md §6.16): the UI pass's GPU time into the frame's sample.
        ui.record_render_stats(&stats);
    }
    let t_wait = std::time::Instant::now();
    renderer
        .context()
        .wait_idle()
        .context("waiting for the GPU")?;
    walker.record_frame_timing(
        t_wait.saturating_duration_since(t_end).as_secs_f64() * 1e3,
        t_wait.elapsed().as_secs_f64() * 1e3,
    );
    Ok(())
}

/// Renders the walker and the UI offscreen once more and reads the frame back (no cursor).
///
/// # Errors
///
/// Renderer errors.
pub fn capture_with_ui(
    renderer: &mut Renderer<'_>,
    walker: &mut Walker,
    ui: &mut ClientUi,
    size: (u32, u32),
) -> anyhow::Result<RgbaImage> {
    let target = renderer.create_offscreen_target(size.0, size.1, OffscreenFormat::Rgba8Srgb)?;
    render_frame(renderer, &target, walker, Some(ui))?;
    Ok(renderer.read_pixels(&target)?)
}

/// The **capture settle** of `ui-scaling.md` §9.3 (revision 15) before a headless
/// screenshot: every surface still in a preview (a change of *Se* waiting for its §4.5 step 4
/// repaint, a §6.1 geometry preview) is settled as if its 150 ms settle time had passed, and
/// settle steps ([`super::UiHost::capture_settle_step`]: the settle request, the workers'
/// arrivals and step 8's paint, its GPU operations applied to `renderer`) run until
/// [`super::UiHost::settled`] holds, for at most [`CAPTURE_SETTLE_STEPS`] steps and
/// [`CAPTURE_SETTLE_LIMIT`].
///
/// **Nothing else changes** (§9.3 items 1 and 2): no plugin code runs (no step 7: no
/// `Update` broadcast, no event delivery, no deferred plugin work), the walker is not updated
/// and the UI's frame clock is not advanced (no frame end), so plugin state, fades, timers,
/// tooltips, floating text and the world stay at the moment the run ended; the steps are not
/// recorded in the U-report or the frame log. Returns whether the UI settled (`true` without
/// an attached UI).
pub fn capture_settle(ui: &mut ClientUi, renderer: &mut Renderer<'_>, rt: &mut Runtime) -> bool {
    if !ui.attached() {
        return true;
    }
    let start = Instant::now();
    for _ in 0..CAPTURE_SETTLE_STEPS {
        let Some(host) = ui.host_mut() else {
            return true;
        };
        if host.capture_settle_step(Some(renderer), &mut rt.plugins) {
            return true;
        }
        if start.elapsed() >= CAPTURE_SETTLE_LIMIT {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    ui.host().is_none_or(super::UiHost::settled)
}

/// Checks that a headless UI run has something that ends it: `--screenshot`,
/// `--duration`, `--ui-script`, `--fly-path`, or a prepared scenario (which ends when the
/// feed finishes, `scenario-feed.md` §12.2).
///
/// # Errors
///
/// When none is given.
pub fn check_end_condition(args: &Args) -> anyhow::Result<()> {
    if args.screenshot.is_none()
        && args.timing_secs().is_none()
        && !args.ui_script
        && args.fly_path.is_none()
        && args.scenario_prepared.is_none()
    {
        bail!(
            "a headless UI run needs --screenshot, --duration, --ui-script, --fly-path or a scenario to end"
        );
    }
    Ok(())
}

/// The walker's screenshot (`render.md` §6.11): with a game UI attached, the world passes,
/// the walker's overlays and the UI pass, without the software cursor; without one, the
/// walker alone.
///
/// # Errors
///
/// Renderer errors.
pub fn capture_frame(
    renderer: &mut Renderer<'_>,
    walker: &mut Walker,
    ui: Option<&mut ClientUi>,
    size: (u32, u32),
) -> anyhow::Result<RgbaImage> {
    match ui.filter(|u| u.attached()) {
        Some(ui) => capture_with_ui(renderer, walker, ui, size),
        None => crate::walker_host::capture(renderer, walker, size),
    }
}

/// Runs the walker offscreen with the game UI (module docs).
///
/// # Errors
///
/// No install, no GPU, the walker or the UI could not start, a timeout, or an output that
/// could not be written.
pub fn run(args: &Args) -> anyhow::Result<UiRunReport> {
    check_end_condition(args)?;
    for path in args
        .screenshot
        .iter()
        .chain(&args.ui_report)
        .chain(&args.frame_log)
    {
        refuse_install_path(args, None, path)?;
    }
    let args2 = script_args(args)?;
    let mut rt = Runtime::start(&ClientUi::runtime_args(&args2), Instant::now())?;
    let shared = rt
        .walker_shared
        .clone()
        .context("walker session not configured")?;
    let mut renderer = Renderer::new_headless(&RendererConfig {
        clear_color: SKY,
        meshes: crate::walker::mesh_config(crate::walker_host::walker_arena_bytes(args)),
        ..RendererConfig::default()
    })
    .context("creating the headless renderer")?;
    let (w, h) = args.size;
    let mut target = renderer.create_offscreen_target(w, h, OffscreenFormat::Rgba8Srgb)?;
    let mut host = WalkerHost::new(args, shared);
    let mut ui = ClientUi::new(&args2);
    ui.prepare(&mut rt);
    let started = Instant::now();
    let flight_secs = args
        .fly_path
        .map_or(0.0, |p| 3.0 * p.length() / args.speed.max(1e-3))
        .min(1.0e6);
    let script_secs = if args.ui_script {
        super::script::UiScript::total_frames() / 30 + super::client::SCRIPT_SETTLE_FRAMES / 30
    } else {
        0
    };
    let deadline = deadline_after(
        started,
        Duration::from_secs(
            args.settle_timeout
                .saturating_add(180)
                .saturating_add(args.timing_secs().unwrap_or(0))
                .saturating_add(script_secs),
        )
        .saturating_add(Duration::from_secs_f64(flight_secs.max(0.0))),
    );
    let mut report = UiRunReport::default();
    let mut last = Instant::now();
    let mut settled_frames = 0u32;
    let mut viewport_set = false;
    let result: anyhow::Result<()> = loop {
        let now = Instant::now();
        let dt = now.saturating_duration_since(last).as_secs_f32();
        last = now;
        let _ = rt.poll_loader();
        rt.tick(now);
        host.poll(&mut rt, &mut renderer);
        if let Some(reason) = &host.failed {
            break Err(anyhow::anyhow!("world walker failed: {reason}"));
        }
        ui.poll(&mut rt, host.walker.as_mut());
        if let Some(e) = ui.failed() {
            break Err(anyhow::anyhow!("game UI failed: {e}"));
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
        for n in &notices {
            walker.notice(n.clone());
        }
        ui.before_update(walker, &mut rt, &mut NoWindow);
        let ev = walker.update(
            &mut renderer,
            rt.world
                .as_mut()
                .map(|w| w as &mut dyn crate::walker::WalkerSession),
            &mut rt.plugins,
            now,
            dt,
        );
        // P5: the dispatcher's `ride`, `findplace` and `leaveinstance`, the feed's mounts
        // for the walker, and the walker's mount state for the plugin views (as `crate::app`
        // runs it, before `after_update`).
        crate::travel::hub::run_p5_requests(&mut rt, walker, now);
        ui.after_update(walker, &mut rt, &ev);
        // The NPC, quest, social and P5 travel/finder/map window frames (decision 0024
        // P3–P5): the same per-frame window updates the windowed path runs.
        super::client::window_frames(&mut ui, &mut rt, walker, &mut renderer, now);
        record_game_windows(&ui, &rt, &mut report);
        // The player and target frames' portraits (P4, P5), before step 8 (as `crate::app`).
        ui.render_combat_portraits(walker, &mut renderer);
        ui.step8(walker, &mut renderer, &mut rt);
        // A renderer error ends the loop through `finish`, which unloads the plugins
        // (decision 0014 D8) and shuts the walker down.
        if let Err(e) = render_frame(&mut renderer, &target, walker, Some(&mut ui)) {
            break Err(e);
        }
        ui.end_frame(&rt);
        report.frames += 1;
        if let Some((sw, sh)) = ui.take_screen_request() {
            // The scripted run's window resize: the render target and the walker's
            // viewport change with the UI's screen (decision 0015 §6).
            match renderer.create_offscreen_target(sw, sh, OffscreenFormat::Rgba8Srgb) {
                Ok(t) => {
                    target = t;
                    walker.set_viewport(sw, sh);
                }
                Err(e) => break Err(e.into()),
            }
        }

        let ui_quiet = ui.host().is_some_and(|h| {
            let s = h.last_sample();
            s.bands == 0 && s.upload_bytes == 0 && h.env().is_none_or(|e| !e.borrow().busy())
        });
        // A prepared scenario ends the run when the feed reports `Finished`
        // (`scenario-feed.md` §12.2); the end line is written on that tick.
        if args.scenario_prepared.is_some()
            && matches!(
                rt.world
                    .as_ref()
                    .and_then(crate::net::World::scenario_status),
                Some(tessera_net::scenario::ScenarioStatus::Finished { .. })
            )
        {
            report.settled = walker.settled();
            break Ok(());
        }
        if args.ui_script {
            if ui.script_done() {
                break Ok(());
            }
        } else if args.fly_path.is_some() {
            if ev.flight_done {
                report.settled = walker.settled();
                break Ok(());
            }
        } else if args.timing_secs().is_some() {
            // `--duration`, or a scenario's `--scenario-timing` (crate::timing).
            if ev.duration_done {
                report.settled = walker.settled();
                break Ok(());
            }
        } else if args.screenshot.is_some() {
            settled_frames = if walker.settled() && ui.attached() && ui_quiet {
                settled_frames + 1
            } else {
                0
            };
            let timed_out = walker.placed()
                && ui.attached()
                && now.saturating_duration_since(started)
                    > Duration::from_secs(args.settle_timeout);
            if settled_frames >= SETTLED_FRAMES || timed_out {
                report.settled = settled_frames >= SETTLED_FRAMES;
                if !report.settled {
                    tracing::warn!("world or UI not settled; taking the screenshot anyway");
                }
                break Ok(());
            }
        }
        if now > deadline {
            break Err(anyhow::anyhow!("timed out"));
        }
        let spent = now.elapsed();
        if spent < FRAME && !args.unpaced {
            std::thread::sleep(FRAME - spent);
        }
    };
    let out = finish(
        args,
        &mut rt,
        &mut host,
        &mut ui,
        &mut renderer,
        &target,
        &mut report,
        result,
    );
    rt.shutdown();
    out.map(|()| report)
}

#[allow(clippy::too_many_arguments)]
fn finish(
    args: &Args,
    rt: &mut Runtime,
    host: &mut WalkerHost,
    ui: &mut ClientUi,
    renderer: &mut Renderer<'_>,
    target: &OffscreenTarget,
    report: &mut UiRunReport,
    result: anyhow::Result<()>,
) -> anyhow::Result<()> {
    let mut first: Option<anyhow::Error> = result.err();
    if let Some(walker) = host.walker.as_mut() {
        if first.is_none()
            && let Some(path) = &args.screenshot
        {
            let r = (|| {
                refuse_install_path(args, Some(&rt.status), path)?;
                // §9.3: the settled picture, never a preview, at the run's last moment.
                if !capture_settle(ui, renderer, rt) {
                    tracing::warn!("UI previews not settled; taking the screenshot anyway");
                }
                let image = capture_with_ui(renderer, walker, ui, args.size)?;
                save_png(path, &image)?;
                tracing::info!(path = %path.display(), settled = report.settled, "UI screenshot saved");
                Ok(())
            })();
            keep(&mut first, r);
        }
        // A scenario timing run whose timed window never started has only load and settle
        // frames in its log (it is reset when the window starts): they are not written as
        // the timed frames.
        let timing_not_started = args.scenario_timing.is_some()
            && !args.ui_script
            && matches!(
                walker.timing_phase(),
                crate::walker::TimingPhase::Waiting | crate::walker::TimingPhase::Off
            );
        if let Some(log) = walker.frame_log() {
            tracing::info!("frame summary: {}", log.summary());
            if timing_not_started {
                if let Some(path) = &args.frame_log {
                    tracing::warn!(
                        path = %path.display(),
                        "the timed window never started; no frame log written"
                    );
                }
            } else if let Some(path) = &args.frame_log {
                let r = refuse_install_path(args, Some(&rt.status), path).and_then(|()| {
                    log.write_csv(path)
                        .with_context(|| format!("writing {}", path.display()))
                });
                keep(&mut first, r);
            }
        }
        if args.scenario_timing.is_some() && !args.ui_script {
            // The scenario timing run (decision 0043 §3): its own report, written to
            // `--ui-report` instead of the scripted run's.
            let t = crate::timing::build(
                walker,
                Some(&*ui),
                rt,
                crate::timing::RunMode::Headless,
                args.size,
                !args.unpaced,
                &renderer.adapter_info().name,
            );
            keep(&mut first, crate::timing::emit(&t, args, Some(&rt.status)));
            if t.window == crate::timing::WindowState::NotStarted {
                // The report says so too; the run fails, as a run without its window.
                keep(
                    &mut first,
                    Err(anyhow::anyhow!(
                        "the timed window never started: the run ended (scenario {}) while it waited for the player to be placed and the area to settle",
                        t.feed.key()
                    )),
                );
            }
            report.timing = Some(t);
        } else if first.is_none() && (args.ui_script || args.ui_report.is_some()) {
            let notes = u2_proxy(renderer, target, walker, ui)
                .map(|(with, without)| {
                    vec![(
                        "U2 proxy: median frame submit+GPU-wait time with / without the UI pass".to_owned(),
                        format!(
                            "{with:.3} ms / {without:.3} ms (difference {:.3} ms; the U2 row above is the timestamp-query figure when the adapter has them)",
                            with - without
                        ),
                    )]
                })
                .unwrap_or_default();
            if let Some(rec) = ui.host().and_then(|h| h.recorder()) {
                let title = format!(
                    "{}x{} offscreen, {}",
                    args.size.0,
                    args.size.1,
                    renderer.adapter_info().name
                );
                let md = report_markdown(rec, &title, &notes);
                tracing::info!("UI budgets:\n{md}");
                if let Some(path) = &args.ui_report {
                    let r = (|| {
                        refuse_install_path(args, Some(&rt.status), path)?;
                        if let Some(d) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
                            std::fs::create_dir_all(d)?;
                        }
                        std::fs::write(path, &md)
                            .with_context(|| format!("writing {}", path.display()))?;
                        Ok(())
                    })();
                    keep(&mut first, r);
                }
                report.report = Some(md);
            }
        }
    }
    report.panels = ui
        .host()
        .and_then(|h| h.panels())
        .map_or(0, |p| p.opened().count());
    report.plugins = rt
        .plugins
        .manager()
        .loaded_plugins()
        .into_iter()
        .map(|p| p.name)
        .collect();
    tracing::info!(windows = ?report.game_windows, "game windows seen");
    ui.shutdown(rt, Some(renderer), &mut NoWindow);
    if let Some(walker) = host.walker.as_mut() {
        walker.shutdown(renderer);
    }
    match first {
        Some(e) => bail!("{e:#}"),
        None => Ok(()),
    }
}

/// The U2 proxy (decision 0015 §6: "where the adapter has them; otherwise not a gate"):
/// the mean wall time of submitting a frame and waiting for the GPU, with and without the
/// UI pass, over [`U2_FRAMES`] frames each.
fn u2_proxy(
    renderer: &mut Renderer<'_>,
    target: &OffscreenTarget,
    walker: &mut Walker,
    ui: &mut ClientUi,
) -> Option<(f64, f64)> {
    // Interleaved, medians of each kind (less sensitive to a busy machine than means).
    let mut with = Vec::new();
    let mut without = Vec::new();
    for i in 0..(2 * U2_FRAMES) {
        let ui_on = i % 2 == 1;
        let t = Instant::now();
        render_frame(renderer, target, walker, ui_on.then_some(&mut *ui)).ok()?;
        let v = t.elapsed().as_secs_f64() * 1e3;
        if ui_on {
            with.push(v);
        } else {
            without.push(v);
        }
    }
    let median = |v: &mut Vec<f64>| {
        v.sort_by(f64::total_cmp);
        v.get(v.len() / 2).copied().unwrap_or(0.0)
    };
    let (with, without) = (median(&mut with), median(&mut without));
    Some((with, without))
}

fn keep(first: &mut Option<anyhow::Error>, r: anyhow::Result<()>) {
    if let Err(e) = r
        && first.is_none()
    {
        *first = Some(e);
    }
}

/// Records the P3/P5 windows open this frame into the run report: the NPC windows
/// (dialogue, vendor, loot, offer, barter), the quest log or tracker, and the travel or
/// instance-finder windows, read through the hubs' UI. Called every frame; a window seen
/// once stays recorded.
fn record_game_windows(ui: &ClientUi, rt: &Runtime, report: &mut UiRunReport) {
    if !ui.attached() {
        return;
    }
    if rt
        .npc
        .ui()
        .is_some_and(|u| u.open_windows().next().is_some())
    {
        report.game_windows.insert("npc");
    }
    if rt
        .quests
        .ui()
        .is_some_and(|u| u.log().is_some() || u.tracker().is_some())
    {
        report.game_windows.insert("quest");
    }
    if let Some(u) = rt.p5.ui() {
        if u.travel().is_some() {
            report.game_windows.insert("travel");
        }
        if u.finder().is_some() {
            report.game_windows.insert("finder");
        }
    }
    if let (Some(u), Some(host)) = (rt.p6.ui(), ui.host()) {
        let names = u.open_windows(&host.ui());
        report.game_windows.extend(names);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser as _;

    #[test]
    fn a_flight_ends_a_headless_ui_run() {
        let a = Args::try_parse_from(["t", "--headless", "--ui", "--fly-path", "1:2,3:4"])
            .expect("args");
        assert!(a.walker_headless() && a.ui_enabled());
        assert!(check_end_condition(&a).is_ok());
        let a = Args::try_parse_from(["t", "--headless", "--ui", "--screenshot", "a.png"])
            .expect("args");
        assert!(check_end_condition(&a).is_ok());
        let a = Args::try_parse_from(["t", "--ui"]).expect("args");
        assert!(check_end_condition(&a).is_err());
    }
}
