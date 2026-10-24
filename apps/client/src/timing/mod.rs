// SPDX-License-Identifier: GPL-3.0-or-later
//! Timing reports of `--scenario-timing` runs (decision 0043 §3, P8 criterion 3).
//!
//! A scenario timing run plays a scenario on the wall clock and, once the player is placed
//! and the area has settled (or `--settle-timeout` has passed), logs every frame for the
//! given number of seconds (the walker's timing run, [`crate::walker::TimingPhase`]). At its
//! end the client builds a [`TimingReport`] from three sources:
//!
//! - the walker's frame log of the timed window (`walker.md` §7): frame-time p50, p95, p99
//!   and maximum, the frames over 16.7 ms ([`FRAME_BUDGET_MS`]) and over 50 ms
//!   ([`STALL_MS`]), and the device underruns during the window when audio plays;
//! - the game UI's recorder ([`crate::ui_host::stats::Recorder`]), split as decision 0043 §3
//!   item 3 (c) says: U1 and U3 over the steady frames of the timed phase ([`TIMED_PHASE`]),
//!   U4 (a) and (b) over its event windows (the NPC windows and the defeat panel opening),
//!   U4 (f) over its content-event frames (new chat lines), U2 where the adapter has
//!   timestamp queries, and U5 over every recorded frame of the run, the settle phase
//!   included ("U5 over the whole run"); U4 (c)–(e) are not applicable to such a run. The
//!   UI's effective scale *Se* is recorded, since (c) holds only at *Se* = 1;
//! - the run itself: surface size, windowed or headless, paced or not, whether the timed
//!   window started and ran its whole duration ([`WindowState`]; a window that never started
//!   has no frame figures), whether the area had settled when it started, whether the
//!   scenario was still playing at its end, and how many frames the frame log or the UI
//!   recorder could not keep because they were full.
//!
//! The report is written as Markdown ([`TimingReport::markdown`]) that ends with a fenced
//! block of `key=value` figures, which [`TimingReport::parse`] reads back (the real-data
//! test of the busiest scenario does). [`TimingReport::budget_failures`] judges the figures
//! against the budgets of decision 0043 §3 item 3; it is called only where the budgets are
//! asserted (`TESSERA_PERF=1`, decision 0010), never by the client itself.

mod report;

pub use report::{
    AudioFigures, FIGURES_HEADER, FRAME_BUDGET_MS, FeedAtEnd, FrameFigures, MAX_BACKEND_CHARS,
    MAX_FIGURE_LINES, MAX_REPORT_BYTES, RunMode, SETTLE_PHASE, STALL_MS, Stat, TIMED_PHASE,
    TimingParseError, TimingReport, UiFigures, WindowState,
};

use crate::ui_host::stats::{FrameSample, Recorder, Summary, steady};
use crate::walker::framelog::{FrameRow, summarize};

impl FeedAtEnd {
    /// From the world session's scenario status.
    #[must_use]
    pub fn of(status: Option<tessera_net::scenario::ScenarioStatus>) -> Self {
        use tessera_net::scenario::ScenarioStatus as S;
        match status {
            Some(S::Running { .. }) => Self::Running,
            Some(S::Finished { .. }) => Self::Finished,
            // `ScenarioStatus` is non-exhaustive: a later status counts as a failure.
            Some(_) => Self::Failed,
            None => Self::None,
        }
    }
}

impl From<Summary> for Stat {
    fn from(s: Summary) -> Self {
        Self {
            n: s.n,
            mean: s.mean,
            p99: s.p99,
            max: s.max,
        }
    }
}

impl FrameFigures {
    /// The figures of logged frames (the percentiles are the frame log summary's,
    /// `walker.md` §7).
    #[must_use]
    pub(crate) fn of_rows(rows: &[FrameRow]) -> Self {
        let s = summarize(rows);
        Self {
            frames: s.frames,
            seconds: s.seconds,
            p50_ms: s.p50_ms,
            p95_ms: s.p95_ms,
            p99_ms: s.p99_ms,
            max_ms: s.max_frame_ms,
            over_frame_budget: rows.iter().filter(|r| r.frame_ms > FRAME_BUDGET_MS).count(),
            over_stall: rows.iter().filter(|r| r.frame_ms > STALL_MS).count(),
            dropped: 0,
        }
    }
}

impl UiFigures {
    /// The figures of the recorder: U1–U4 over its frames of the timed phase
    /// ([`TIMED_PHASE`]), U5 over all its frames (the whole run), with the UI's effective
    /// scale `ui_scale` (*Se*) and whether the surface limited it.
    #[must_use]
    pub fn of_recorder(rec: &Recorder, ui_scale: f64, ui_scale_limited: bool) -> Self {
        let frames: Vec<&FrameSample> = rec.phase(Some(TIMED_PHASE)).collect();
        let steady_frames = || frames.iter().copied().filter(|s| steady(s));
        let u1 = Summary::of(steady_frames().map(|s| s.ui_ms));
        let u2 = Summary::of(steady_frames().filter_map(|s| s.ui_gpu_ms));
        let u3 = Summary::of(steady_frames().map(|s| s.upload_bytes as f64));
        let mut out = Self {
            frames: frames.len(),
            ui_scale,
            ui_scale_limited,
            dropped: rec.dropped,
            u1: u1.into(),
            u1_steps_ms: [
                Summary::of(steady_frames().map(|s| s.step1_ms)).mean,
                Summary::of(steady_frames().map(|s| s.step7_ms)).mean,
                Summary::of(steady_frames().map(|s| s.paint_ms)).mean,
            ],
            u2: u2.into(),
            u2_fed: rec.render_stats_fed,
            u3_mean_bytes: u3.mean,
            ..Self::default()
        };
        for s in &frames {
            out.u3_max_bytes = out.u3_max_bytes.max(s.upload_bytes);
            if s.event || s.event_window {
                out.event_frames += 1;
                out.u4a_ms = out.u4a_ms.max(s.frame_ms);
                out.u4b_ms = out.u4b_ms.max(s.paint_ms);
            }
            if s.content_event {
                out.content_frames += 1;
                out.u4f_ms = out.u4f_ms.max(s.ui_ms);
            }
        }
        // U5 over the whole run (decision 0043 §3 item 3 (c)): the settle phase too, which
        // holds the first paints of the panels and the world load.
        for s in rec.phase(None) {
            out.canvas_bytes = out.canvas_bytes.max(s.canvas_bytes);
            out.texture_bytes = out.texture_bytes.max(s.texture_bytes);
            out.unpinned_bytes = out
                .unpinned_bytes
                .max(s.image_bytes.saturating_sub(s.pinned_bytes));
            out.pinned_bytes = out.pinned_bytes.max(s.pinned_bytes);
        }
        out
    }
}

/// Builds the report of a timing run at its end, from the walker's frame log, the game
/// UI's recorder (when it recorded) and the world session's scenario status. When the
/// walker's timed window never started, the frame log holds only load and settle frames,
/// so the report has no frame figures ([`WindowState::NotStarted`]).
#[must_use]
pub(crate) fn build(
    walker: &crate::walker::Walker,
    ui: Option<&crate::ui_host::client::ClientUi>,
    rt: &crate::runtime::Runtime,
    mode: RunMode,
    surface: (u32, u32),
    paced: bool,
    adapter: &str,
) -> TimingReport {
    use crate::walker::TimingPhase;
    let (window, settled) = match walker.timing_phase() {
        TimingPhase::Running { settled } => (WindowState::Running, settled),
        TimingPhase::Done { settled } => (WindowState::Done, settled),
        TimingPhase::Off | TimingPhase::Waiting => (WindowState::NotStarted, false),
    };
    // The frame log is reset when the window starts; before that it holds the load and
    // settle frames, which are not timed frames.
    let log = walker
        .frame_log()
        .filter(|_| window != WindowState::NotStarted);
    let frames = log.map_or_else(FrameFigures::default, |l| FrameFigures {
        dropped: l.dropped(),
        ..FrameFigures::of_rows(l.rows())
    });
    let underruns = log.map_or(0, |l| l.summary().audio_underruns);
    let audio = walker.audio().and_then(|a| {
        let r = a.report();
        r.stats.backend.map(|b| AudioFigures {
            backend: backend_key(b).to_owned(),
            underruns,
        })
    });
    let feed = FeedAtEnd::of(
        rt.world
            .as_ref()
            .and_then(crate::net::World::scenario_status),
    );
    let ui = ui.and_then(|u| u.host()).and_then(|h| {
        let scale = h.scale();
        h.recorder()
            .map(|r| UiFigures::of_recorder(r, scale.effective_scale(), scale.is_limited()))
    });
    TimingReport {
        title: format!(
            "{} × {} {}, {}, {}",
            surface.0,
            surface.1,
            mode.key(),
            if paced { "paced" } else { "unpaced" },
            adapter
        ),
        mode,
        surface,
        paced,
        window,
        settled,
        feed,
        frames,
        audio,
        ui,
    }
}

/// The report's word for an audio backend (stable, lower case; budget (d) is measured only
/// with `device`).
#[must_use]
pub fn backend_key(b: tessera_audio::engine::BackendKind) -> &'static str {
    use tessera_audio::engine::BackendKind as K;
    match b {
        K::Disabled => "disabled",
        K::Null => "null",
        K::Offline => "offline",
        K::Device => "device",
    }
}

/// Logs a timing report and writes it to `--ui-report` when given (refused inside the
/// install).
///
/// # Errors
///
/// When the report file cannot be written.
pub(crate) fn emit(
    report: &TimingReport,
    args: &crate::cli::Args,
    status: Option<&crate::runtime::DataStatus>,
) -> anyhow::Result<()> {
    let md = report.markdown();
    tracing::info!("scenario timing report:\n{md}");
    if let Some(path) = &args.ui_report {
        crate::runtime::refuse_install_path(args, status, path)?;
        if let Some(d) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(d)?;
        }
        std::fs::write(path, &md)
            .map_err(|e| anyhow::anyhow!("writing {}: {e}", path.display()))?;
        tracing::info!(path = %path.display(), "timing report written");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_host::stats::FrameSample;

    fn rows(times: &[f64]) -> Vec<FrameRow> {
        let mut t = 0.0;
        times
            .iter()
            .map(|&ms| {
                t += ms / 1e3;
                FrameRow {
                    t,
                    frame_ms: ms,
                    ..FrameRow::default()
                }
            })
            .collect()
    }

    fn report(paced: bool, times: &[f64], ui: Option<UiFigures>) -> TimingReport {
        TimingReport {
            title: "test".into(),
            mode: RunMode::Windowed,
            surface: (1920, 1080),
            paced,
            window: WindowState::Done,
            settled: true,
            feed: FeedAtEnd::Running,
            frames: FrameFigures::of_rows(&rows(times)),
            audio: Some(AudioFigures {
                backend: "device".into(),
                underruns: 0,
            }),
            ui,
        }
    }

    #[test]
    fn frame_figures_count_frames_over_the_budgets() {
        let f = FrameFigures::of_rows(&rows(&[10.0, 16.7, 16.8, 20.0, 50.0, 50.1, 8.0]));
        assert_eq!(f.frames, 7);
        assert_eq!(f.over_frame_budget, 4, "16.7 itself is within the budget");
        assert_eq!(f.over_stall, 1, "50 ms itself is not a stall");
        assert_eq!(f.max_ms, 50.1);
        assert_eq!(FrameFigures::of_rows(&[]), FrameFigures::default());
    }

    #[test]
    fn ui_figures_take_only_the_timed_frames_and_split_them() {
        let mut rec = Recorder::default();
        let s = |ui_ms: f64| FrameSample {
            ui_ms,
            frame_ms: 16.0,
            ..FrameSample::default()
        };
        rec.push(SETTLE_PHASE, s(9.0));
        rec.push(TIMED_PHASE, s(0.2));
        rec.push(TIMED_PHASE, s(0.4));
        rec.push(
            TIMED_PHASE,
            FrameSample {
                content_event: true,
                ..s(3.0)
            },
        );
        rec.push(
            TIMED_PHASE,
            FrameSample {
                event: true,
                paint_ms: 2.5,
                frame_ms: 40.0,
                image_bytes: 100,
                pinned_bytes: 30,
                ..s(5.0)
            },
        );
        let u = UiFigures::of_recorder(&rec, 1.0, false);
        assert_eq!(u.frames, 4);
        assert_eq!(u.u1.n, 2, "only steady timed frames");
        assert!((u.u1.mean - 0.3).abs() < 1e-9);
        assert_eq!(u.event_frames, 1);
        assert_eq!(u.u4a_ms, 40.0);
        assert_eq!(u.u4b_ms, 2.5);
        assert_eq!(u.content_frames, 1);
        assert_eq!(u.u4f_ms, 3.0);
        assert_eq!(u.unpinned_bytes, 70);
        assert_eq!(u.pinned_bytes, 30);
        assert_eq!(u.u2.n, 0);
    }

    #[test]
    fn the_markdown_figures_read_back() {
        let mut rec = Recorder::default();
        rec.push(
            TIMED_PHASE,
            FrameSample {
                ui_ms: 0.1,
                ui_gpu_ms: Some(0.05),
                ..FrameSample::default()
            },
        );
        let r = report(
            true,
            &[16.6, 16.7, 17.0],
            Some(UiFigures::of_recorder(&rec, 1.0, false)),
        );
        let md = r.markdown();
        assert!(md.starts_with("### test\n"));
        assert!(md.contains("| Frames over 50 ms | 0 |"));
        let back = TimingReport::parse(&md).expect("parse");
        assert_eq!(back.mode, r.mode);
        assert_eq!(back.surface, r.surface);
        assert_eq!(back.frames.frames, 3);
        assert_eq!(back.frames.over_frame_budget, 1);
        assert_eq!(back.audio, r.audio);
        let (a, b) = (back.ui.expect("ui"), r.ui.expect("ui"));
        assert_eq!(a.u2.n, 1);
        assert_eq!(a.frames, b.frames);
        assert_eq!(a.u2_fed, b.u2_fed);
        // Without the UI and without audio.
        let mut plain = report(false, &[10.0], None);
        plain.audio = None;
        let back = TimingReport::parse(&plain.markdown()).expect("parse");
        assert!(back.ui.is_none() && back.audio.is_none() && !back.paced);
    }

    #[test]
    fn hostile_reports_are_refused_without_panics() {
        assert_eq!(
            TimingReport::parse("no block"),
            Err(TimingParseError::NoFigures)
        );
        assert_eq!(
            TimingReport::parse(&format!("{FIGURES_HEADER}\nmode=windowed\n")),
            Err(TimingParseError::Unclosed)
        );
        assert!(matches!(
            TimingReport::parse(&format!("{FIGURES_HEADER}\nmode\n```")),
            Err(TimingParseError::Figure(_))
        ));
        assert!(matches!(
            TimingReport::parse(&format!("{FIGURES_HEADER}\nmode=windowed\n```")),
            Err(TimingParseError::Figure(_))
        ));
        let big = "x".repeat(MAX_REPORT_BYTES + 1);
        assert_eq!(TimingReport::parse(&big), Err(TimingParseError::TooLarge));
        // Every figure present but one value broken.
        let md = report(true, &[10.0], None)
            .markdown()
            .replace("p99_ms=", "p99_ms=NaN");
        assert!(matches!(
            TimingReport::parse(&md),
            Err(TimingParseError::Figure(k)) if k == "p99_ms"
        ));
    }

    #[test]
    fn budgets_follow_the_paced_and_unpaced_rules() {
        let ok_ui = UiFigures {
            ui_scale: 1.0,
            u1: Stat {
                n: 10,
                mean: 0.2,
                p99: 0.9,
                max: 1.5,
            },
            ..UiFigures::default()
        };
        assert!(
            report(true, &[16.7; 10], Some(ok_ui))
                .budget_failures()
                .is_empty()
        );
        // A paced run is not judged by p99 (its intervals sit at the vsync period).
        assert!(
            report(true, &[24.0; 10], Some(ok_ui))
                .budget_failures()
                .is_empty()
        );
        let stall = report(true, &[16.7, 51.0], Some(ok_ui)).budget_failures();
        assert_eq!(stall.len(), 1, "{stall:?}");
        assert!(
            report(false, &[10.0; 100], None)
                .budget_failures()
                .is_empty()
        );
        assert_eq!(report(false, &[20.0; 100], None).budget_failures().len(), 1);
        let slow_ui = UiFigures {
            ui_scale: 1.0,
            u1: Stat {
                n: 10,
                mean: 0.5,
                p99: 0.9,
                max: 1.0,
            },
            content_frames: 3,
            u4f_ms: 5.0,
            ..UiFigures::default()
        };
        assert_eq!(
            report(true, &[16.7], Some(slow_ui)).budget_failures().len(),
            2
        );
        let mut underrun = report(false, &[10.0], None);
        underrun.audio = Some(AudioFigures {
            backend: "device".into(),
            underruns: 2,
        });
        assert_eq!(underrun.budget_failures().len(), 1);
    }

    #[test]
    fn ui_figures_take_u5_over_the_whole_run() {
        let mut rec = Recorder::default();
        rec.push(
            SETTLE_PHASE,
            FrameSample {
                canvas_bytes: 900,
                texture_bytes: 800,
                image_bytes: 700,
                pinned_bytes: 100,
                ..FrameSample::default()
            },
        );
        rec.push(
            TIMED_PHASE,
            FrameSample {
                canvas_bytes: 10,
                ..FrameSample::default()
            },
        );
        let u = UiFigures::of_recorder(&rec, 0.919, true);
        assert_eq!(u.frames, 1, "U1–U4 count only the timed frames");
        assert_eq!(
            (
                u.canvas_bytes,
                u.texture_bytes,
                u.unpinned_bytes,
                u.pinned_bytes
            ),
            (900, 800, 600, 100),
            "the settle phase's peaks count for U5"
        );
        assert!(u.ui_scale_limited && (u.ui_scale - 0.919).abs() < 1e-12);
    }

    #[test]
    fn a_full_recorder_counts_and_reports_its_dropped_frames() {
        let mut rec = Recorder::default();
        for _ in 0..crate::ui_host::stats::MAX_RECORDED_FRAMES + 3 {
            rec.push(TIMED_PHASE, FrameSample::default());
        }
        assert_eq!(rec.dropped, 3);
        let u = UiFigures {
            u1: Stat {
                n: 10,
                ..Stat::default()
            },
            ..UiFigures::of_recorder(&rec, 1.0, false)
        };
        assert_eq!(u.dropped, 3);
        let mut r = report(true, &[16.7; 10], Some(u));
        r.frames.dropped = 2;
        let md = r.markdown();
        assert!(md.contains("3 frames not recorded"), "{md}");
        assert!(md.contains("2 frames not logged"), "{md}");
        let back = TimingReport::parse(&md).expect("parse");
        assert_eq!(back.frames.dropped, 2);
        assert_eq!(back.ui.map(|u| u.dropped), Some(3));
        let fails = r.budget_failures();
        assert_eq!(fails.len(), 2, "{fails:?}");
        assert!(fails.iter().any(|f| f.contains("frame log was full")));
        assert!(fails.iter().any(|f| f.contains("UI recorder was full")));
    }

    #[test]
    fn a_window_that_never_started_is_reported_and_fails() {
        let mut r = report(true, &[], None);
        r.window = WindowState::NotStarted;
        r.settled = false;
        r.feed = FeedAtEnd::Finished;
        let md = r.markdown();
        assert!(md.contains("never started"), "{md}");
        assert!(!md.contains("settle timeout"), "{md}");
        let back = TimingReport::parse(&md).expect("parse");
        assert_eq!(back.window, WindowState::NotStarted);
        assert_eq!(
            back.budget_failures(),
            vec!["no timed window: the run ended before the window started".to_owned()]
        );
        // A window that started but ended before its duration fails too.
        let mut early = report(false, &[10.0; 10], None);
        early.window = WindowState::Running;
        assert!(early.markdown().contains("ended early"));
        let fails = early.budget_failures();
        assert!(
            fails[0].starts_with("the timed window ended early"),
            "{fails:?}"
        );
    }

    #[test]
    fn budget_c_needs_se_1_and_budget_d_the_device() {
        let ui = |ui_scale, limited| UiFigures {
            ui_scale,
            ui_scale_limited: limited,
            u1: Stat {
                n: 10,
                mean: 3.0,
                p99: 6.0,
                max: 9.0,
            },
            ..UiFigures::default()
        };
        // At Se = 0.919 (a 941 × 1041 surface) (c) is not judged: one failure, no U1 row.
        let fails = report(true, &[16.7; 10], Some(ui(0.919, true))).budget_failures();
        assert_eq!(fails.len(), 1, "{fails:?}");
        assert!(
            fails[0].starts_with("(c) not judged: Se = 0.9190"),
            "{fails:?}"
        );
        assert!(fails[0].contains("1024 × 768"));
        // At Se = 1 the U1 budget is judged.
        let fails = report(true, &[16.7; 10], Some(ui(1.0, false))).budget_failures();
        assert_eq!(fails.len(), 1, "{fails:?}");
        assert!(fails[0].starts_with("(c) U1"), "{fails:?}");
        // No steady UI frame: (c) not measured.
        let mut empty = ui(1.0, false);
        empty.u1.n = 0;
        let fails = report(true, &[16.7; 10], Some(empty)).budget_failures();
        assert!(fails[0].starts_with("(c) not measured"), "{fails:?}");
        // The null backend's 0 underruns do not pass (d).
        let mut null = report(false, &[10.0; 10], None);
        null.audio = Some(AudioFigures {
            backend: "null".into(),
            underruns: 0,
        });
        let fails = null.budget_failures();
        assert_eq!(fails.len(), 1, "{fails:?}");
        assert!(fails[0].starts_with("(d) not measured"), "{fails:?}");
        null.audio = None;
        assert!(null.budget_failures()[0].starts_with("(d) not measured"));
        assert_eq!(
            backend_key(tessera_audio::engine::BackendKind::Device),
            "device"
        );
        assert_eq!(
            backend_key(tessera_audio::engine::BackendKind::Null),
            "null"
        );
    }

    /// The fuzz target's round trip: a backend whose 64th character is white space used to
    /// be cut to a value ending in white space, which did not read back the same.
    #[test]
    fn backends_that_would_not_read_back_are_refused() {
        let base = report(false, &[10.0], None).markdown();
        let with = |b: &str| base.replace("audio_backend=device", &format!("audio_backend={b}"));
        for bad in [
            format!("{} b", "a".repeat(63)),
            format!("{}\u{a0}y", "x".repeat(63)),
            format!("none{}x", " ".repeat(60)),
            "a b".to_owned(),
            "a".repeat(MAX_BACKEND_CHARS + 1),
        ] {
            assert_eq!(
                TimingReport::parse(&with(&bad)),
                Err(TimingParseError::Figure("audio_backend".into())),
                "{bad:?}"
            );
        }
        let ok = "a".repeat(MAX_BACKEND_CHARS);
        let r = TimingReport::parse(&with(&ok)).expect("a 64-character word");
        let back = TimingReport::parse(&r.markdown()).expect("reads back");
        assert_eq!(back.figures(), r.figures());
        // Values that end the line in white space are trimmed with it and still read back.
        let r = TimingReport::parse(&with("device  ")).expect("trailing space");
        assert_eq!(r.audio.map(|a| a.backend), Some("device".into()));
    }
}
