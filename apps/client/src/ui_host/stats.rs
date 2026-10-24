// SPDX-License-Identifier: GPL-3.0-or-later
//! Per-frame measurements of the game UI against the budgets U1–U5 of decision 0015 §6.
//!
//! The host times its own work in frame-loop steps 1, 7 and 8 and subtracts the plugin
//! host's Lua time measured in the same spans (`PluginManager::take_plugin_time`), so U1
//! counts only Tessera's UI work; plugin time is kept apart. Each frame is classified as
//! steady state, an **event frame** (panel open, screen resize, scale change, resize-bar
//! drag; U4) or a **content-event frame** (a scroll step or a new chat line; U4 (f)), as
//! the host or the scripted run marks it.
//!
//! An event's **window** is its trigger frame plus the frames that continue its jobs: the
//! host keeps marking frames until the UI is quiet again (no band painted, nothing
//! uploaded, no image or font job in flight), at most [`super::MAX_EVENT_WINDOW_FRAMES`]
//! (the U4 (e) bound). Steady state (U1, U3's mean) excludes every frame of a window;
//! U4 (a) and (b) are judged over all of them. U4 (c)–(e) (show-to-first-draw and job
//! latencies) are not measured yet and are reported as such.
//!
//! Each frame also carries the paint step's `ui-scaling.md` §8 figures: the paint jobs on the
//! asset workers (S1, [`WorkerSample`]) and the glyph derivation on the frame thread and the
//! ladder steps from the workers (S7, [`GlyphSample`]), copied from
//! `tessera_ui::paint::PaintFrameStats`.

use std::time::Duration;

use tessera_ui::paint::PaintFrameStats;

/// One frame's paint jobs on the asset workers (`ui-scaling.md` §8 S1).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WorkerSample {
    /// Jobs handed to the workers.
    pub jobs_started: u32,
    /// Worker jobs that came back.
    pub jobs_completed: u32,
    /// Damage pixels of the jobs handed over.
    pub pixels: u64,
    /// Worker jobs that failed (repainted on the frame thread).
    pub failures: u32,
}

impl WorkerSample {
    /// The figures of a frame's paint step.
    #[must_use]
    pub fn of(s: &PaintFrameStats) -> Self {
        Self {
            jobs_started: s.worker_jobs_started,
            jobs_completed: s.worker_jobs_completed,
            pixels: s.worker_pixels,
            failures: s.worker_failures,
        }
    }
}

/// One frame's glyph derivation (`ui-scaling.md` §6.5, §8 S7).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GlyphSample {
    /// Glyphs derived on the frame thread.
    pub derived: u32,
    /// Nanoseconds laying out and deriving scaled text (part of the paint slice).
    pub derive_ns: u64,
    /// Nanoseconds of the derivations alone on the frame thread.
    pub derivation_ns: u64,
    /// The longest single derivation on the frame thread, nanoseconds.
    pub derivation_max_ns: u64,
    /// Ladder steps from the workers accepted in the frame.
    pub ladder_steps: u32,
    /// Their worker time, nanoseconds.
    pub ladder_step_ns: u64,
    /// The longest of them, nanoseconds.
    pub ladder_step_max_ns: u64,
}

impl GlyphSample {
    /// The figures of a frame's paint step.
    #[must_use]
    pub fn of(s: &PaintFrameStats) -> Self {
        Self {
            derived: s.glyphs_derived,
            derive_ns: s.glyph_derive_ns,
            derivation_ns: s.glyph_derivation_ns,
            derivation_max_ns: s.glyph_derivation_max_ns,
            ladder_steps: s.ladder_steps,
            ladder_step_ns: s.ladder_step_ns,
            ladder_step_max_ns: s.ladder_step_max_ns,
        }
    }
}

/// S1 over a run: (jobs handed to the workers, their damage pixels, failures).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct S1Summary {
    /// Jobs handed to the workers.
    pub jobs: u64,
    /// Their damage pixels.
    pub pixels: u64,
    /// Failed worker jobs.
    pub failures: u64,
}

/// S7 over a run.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct S7Summary {
    /// Glyphs derived on the frame thread.
    pub glyphs: u64,
    /// Mean derivation time per glyph on the frame thread, microseconds (0 without glyphs).
    pub mean_us: f64,
    /// The longest single derivation, microseconds.
    pub max_us: f64,
    /// Ladder steps accepted from the workers.
    pub steps: u64,
    /// Mean worker time per step, milliseconds (0 without steps).
    pub step_mean_ms: f64,
    /// The longest step, milliseconds.
    pub step_max_ms: f64,
}

/// One frame's figures.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FrameSample {
    /// Tessera's UI main-thread time (U1), milliseconds.
    pub ui_ms: f64,
    /// Of that, the step-8 paint (layout, paint, damage, upload encoding), milliseconds.
    pub paint_ms: f64,
    /// Host time in step 1 (offers, plugin input handlers included), milliseconds.
    pub step1_ms: f64,
    /// Host time in step 7 (plugin work included), milliseconds.
    pub step7_ms: f64,
    /// Plugin Lua time in steps 1 and 7, milliseconds.
    pub plugin_ms: f64,
    /// Whole frame time (pacing excluded), milliseconds.
    pub frame_ms: f64,
    /// UI upload bytes (U3).
    pub upload_bytes: u64,
    /// CPU canvas bytes after the frame (U5).
    pub canvas_bytes: u64,
    /// GPU surface texture bytes after the frame (U5).
    pub texture_bytes: u64,
    /// Decoded-image bytes held (U5).
    pub image_bytes: u64,
    /// Pinned decoded-image bytes (U5).
    pub pinned_bytes: u64,
    /// An event's trigger frame (U4 (a)–(e)).
    pub event: bool,
    /// A frame inside an event window: the trigger frame or a frame continuing its jobs
    /// (module docs).
    pub event_window: bool,
    /// A content-event frame (U4 (f)).
    pub content_event: bool,
    /// Pointer events offered this frame.
    pub pointer_events: u32,
    /// Composition entries drawn.
    pub entries: u32,
    /// Bands painted.
    pub bands: u32,
    /// GPU time of the UI pass (U2, render.md §6.16), milliseconds, from timestamp queries:
    /// the most recent resolved figure when the frame was submitted; `None` without
    /// `TIMESTAMP_QUERY` or before the first query resolved.
    pub ui_gpu_ms: Option<f64>,
    /// Paint jobs on the asset workers (S1).
    pub worker: WorkerSample,
    /// Glyph derivation and ladder steps (S7).
    pub glyphs: GlyphSample,
}

/// Percentile of sorted values (nearest rank); 0 for none.
#[must_use]
pub fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = ((p / 100.0) * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

/// Mean, p99 and maximum of a set of values.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Summary {
    /// Number of values.
    pub n: usize,
    /// Mean.
    pub mean: f64,
    /// 99th percentile.
    pub p99: f64,
    /// Maximum.
    pub max: f64,
}

impl Summary {
    /// Summarises `values`.
    #[must_use]
    pub fn of(values: impl IntoIterator<Item = f64>) -> Self {
        let mut v: Vec<f64> = values.into_iter().filter(|x| x.is_finite()).collect();
        if v.is_empty() {
            return Self::default();
        }
        v.sort_by(f64::total_cmp);
        let n = v.len();
        Self {
            n,
            mean: v.iter().sum::<f64>() / n as f64,
            p99: percentile(&v, 99.0),
            max: v[n - 1],
        }
    }
}

/// A recorded phase of frames (module docs).
#[derive(Debug, Clone, Default)]
pub struct Recorder {
    /// Frames, in order, with their phase label.
    pub frames: Vec<(&'static str, FrameSample)>,
    /// Whether the run fed the renderer's frame figures to the UI host at least once
    /// (`UiHost::record_render_stats`). Without that, a U2 row with no frames says nothing
    /// about the adapter's timestamp queries.
    pub render_stats_fed: bool,
    /// Frames not kept because the recorder was full ([`MAX_RECORDED_FRAMES`]); reports
    /// that judge the frames say so when this is not 0.
    pub dropped: usize,
}

/// Most frames a recorder keeps (about 30 minutes at 60 Hz).
pub const MAX_RECORDED_FRAMES: usize = 120_000;

impl Recorder {
    /// Adds a frame (counted in [`Recorder::dropped`] instead once the recorder holds
    /// [`MAX_RECORDED_FRAMES`]).
    pub fn push(&mut self, phase: &'static str, s: FrameSample) {
        if self.frames.len() < MAX_RECORDED_FRAMES {
            self.frames.push((phase, s));
        } else {
            self.dropped = self.dropped.saturating_add(1);
        }
    }

    /// The frames of one phase (all with `None`).
    pub fn phase<'a>(&'a self, phase: Option<&'a str>) -> impl Iterator<Item = &'a FrameSample> {
        self.frames
            .iter()
            .filter(move |(p, _)| phase.is_none_or(|w| *p == w))
            .map(|(_, s)| s)
    }

    /// U1 over the steady-state frames of a phase.
    #[must_use]
    pub fn u1(&self, phase: Option<&str>) -> Summary {
        Summary::of(self.phase(phase).filter(|s| steady(s)).map(|s| s.ui_ms))
    }

    /// U2 over the steady-state frames that carry a GPU time (`UiStats::gpu_ns`, render.md
    /// §6.16): `n` is 0 when no frame did (no timestamp queries, none resolved, or the run
    /// never fed the renderer's figures, [`Recorder::render_stats_fed`]).
    #[must_use]
    pub fn u2(&self) -> Summary {
        Summary::of(
            self.phase(None)
                .filter(|s| steady(s))
                .filter_map(|s| s.ui_gpu_ms),
        )
    }

    /// U3: steady-state mean and the overall maximum of the upload bytes.
    #[must_use]
    pub fn u3(&self) -> (f64, u64) {
        let steady = Summary::of(
            self.phase(None)
                .filter(|s| steady(s))
                .map(|s| s.upload_bytes as f64),
        );
        let max = self.phase(None).map(|s| s.upload_bytes).max().unwrap_or(0);
        (steady.mean, max)
    }

    /// U4: (longest frame in an event window, longest paint in an event window, longest
    /// UI time in content-event frames), milliseconds.
    #[must_use]
    pub fn u4(&self) -> (f64, f64, f64) {
        let ev = self.phase(None).filter(|s| s.event || s.event_window);
        let (mut frame, mut paint) = (0.0f64, 0.0f64);
        for s in ev {
            frame = frame.max(s.frame_ms);
            paint = paint.max(s.paint_ms);
        }
        let content = self
            .phase(None)
            .filter(|s| s.content_event)
            .map(|s| s.ui_ms)
            .fold(0.0f64, f64::max);
        (frame, paint, content)
    }

    /// U5: the largest canvas, texture, image and pinned-image bytes seen.
    #[must_use]
    pub fn u5(&self) -> (u64, u64, u64, u64) {
        let mut m = (0u64, 0u64, 0u64, 0u64);
        for s in self.phase(None) {
            m.0 = m.0.max(s.canvas_bytes);
            m.1 = m.1.max(s.texture_bytes);
            m.2 = m.2.max(s.image_bytes);
            m.3 = m.3.max(s.pinned_bytes);
        }
        m
    }

    /// S1 over every frame: the paint jobs handed to the asset workers.
    #[must_use]
    pub fn s1(&self) -> S1Summary {
        let mut out = S1Summary::default();
        for f in self.phase(None) {
            out.jobs += u64::from(f.worker.jobs_started);
            out.pixels = out.pixels.saturating_add(f.worker.pixels);
            out.failures += u64::from(f.worker.failures);
        }
        out
    }

    /// S7 over every frame: the glyph derivations on the frame thread and the ladder steps
    /// from the workers.
    #[must_use]
    pub fn s7(&self) -> S7Summary {
        let (mut glyphs, mut ns, mut max_ns) = (0u64, 0u64, 0u64);
        let (mut steps, mut step_ns, mut step_max) = (0u64, 0u64, 0u64);
        for f in self.phase(None) {
            let g = &f.glyphs;
            glyphs += u64::from(g.derived);
            ns = ns.saturating_add(g.derivation_ns);
            max_ns = max_ns.max(g.derivation_max_ns);
            steps += u64::from(g.ladder_steps);
            step_ns = step_ns.saturating_add(g.ladder_step_ns);
            step_max = step_max.max(g.ladder_step_max_ns);
        }
        S7Summary {
            glyphs,
            mean_us: if glyphs > 0 {
                ns as f64 / glyphs as f64 / 1e3
            } else {
                0.0
            },
            max_us: max_ns as f64 / 1e3,
            steps,
            step_mean_ms: if steps > 0 {
                step_ns as f64 / steps as f64 / 1e6
            } else {
                0.0
            },
            step_max_ms: step_max as f64 / 1e6,
        }
    }

    /// Plugin time summary.
    #[must_use]
    pub fn plugins(&self) -> Summary {
        Summary::of(self.phase(None).map(|s| s.plugin_ms))
    }

    /// Distinct phase labels, in first-seen order.
    #[must_use]
    pub fn phases(&self) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = Vec::new();
        for (p, _) in &self.frames {
            if !out.contains(p) {
                out.push(p);
            }
        }
        out
    }
}

/// Whether a frame is steady state: outside every event window and not a content-event
/// frame.
#[must_use]
pub fn steady(s: &FrameSample) -> bool {
    !s.event && !s.event_window && !s.content_event
}

/// Milliseconds of a duration.
#[must_use]
pub fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1e3
}

/// The U1–U5 report of a recorded run as Markdown (the table of `docs/perf/m5-ui-budgets.md`).
/// `notes` are extra (label, value) rows, for example the U2 proxy.
#[must_use]
pub fn report_markdown(rec: &Recorder, title: &str, notes: &[(String, String)]) -> String {
    use std::fmt::Write as _;
    let mut s = String::new();
    let _ = writeln!(s, "### {title}\n");
    let _ = writeln!(s, "| Measure | Phase | Value | Budget |");
    let _ = writeln!(s, "|---|---|---|---|");
    for phase in ["walk", "sweep", "sweep1000"] {
        let u = rec.u1(Some(phase));
        let _ = writeln!(
            s,
            "| U1 UI main-thread time (steady) | {phase} | mean {:.3} ms, p99 {:.3} ms, max {:.3} ms ({} frames) | mean ≤ 0.3 ms, p99 ≤ 1.0 ms |",
            u.mean, u.p99, u.max, u.n
        );
    }
    let all = rec.u1(None);
    let _ = writeln!(
        s,
        "| U1 UI main-thread time (steady) | all | mean {:.3} ms, p99 {:.3} ms ({} frames) | — |",
        all.mean, all.p99, all.n
    );
    let u2 = rec.u2();
    if u2.n > 0 {
        let _ = writeln!(
            s,
            "| U2 UI pass GPU time (timestamp queries, steady) | all | mean {:.3} ms, p99 {:.3} ms, max {:.3} ms ({} frames; counted per frame: a frame that repeats the last resolved figure counts again) | ≤ 0.3 ms at 3840 × 2160 |",
            u2.mean, u2.p99, u2.max, u2.n
        );
    } else {
        // Only a run that fed the renderer's figures can blame the adapter; even then a
        // short run may end before the first query resolved.
        let why = if rec.render_stats_fed {
            "no UI GPU time recorded: the adapter has no timestamp queries, or none resolved in the steady frames"
        } else {
            "no UI GPU time recorded: this run did not feed the renderer's frame figures to the UI host"
        };
        let _ = writeln!(
            s,
            "| U2 UI pass GPU time (timestamp queries, steady) | — | not measured ({why}) | ≤ 0.3 ms at 3840 × 2160 (not a gate when not measured) |"
        );
    }
    let (mean_up, max_up) = rec.u3();
    let _ = writeln!(
        s,
        "| U3 UI upload bytes | steady mean / max | {:.1} KiB / {:.1} KiB | mean ≤ 256 KiB, cap 8 MiB |",
        mean_up / 1024.0,
        max_up as f64 / 1024.0
    );
    let (frame, paint, content) = rec.u4();
    let _ = writeln!(
        s,
        "| U4 (a) longest frame in an event window | event windows | {frame:.1} ms | ≤ 50 ms |"
    );
    let _ = writeln!(
        s,
        "| U4 (b) longest UI paint in an event window | event windows | {paint:.2} ms | ≤ 4 ms |"
    );
    let _ = writeln!(
        s,
        "| U4 (c) first draw within 100 ms of being shown | — | not measured | ≤ 100 ms |"
    );
    let _ = writeln!(
        s,
        "| U4 (d) each job of a surface up to 1038 × 802 within 4 frames | — | not measured | ≤ 4 frames |"
    );
    let _ = writeln!(
        s,
        "| U4 (e) first job of the screen-sized window within 40 frames | — | not measured | ≤ 40 frames |"
    );
    let _ = writeln!(
        s,
        "| U4 (f) longest UI time in a content-event frame | content frames | {content:.2} ms | ≤ 4 ms |"
    );
    let (canvas, tex, img, pinned) = rec.u5();
    let mib = |b: u64| b as f64 / (1024.0 * 1024.0);
    let _ = writeln!(
        s,
        "| U5 memory (peak) | all | canvases {:.1} MiB, textures {:.1} MiB, images {:.1} MiB (pinned {:.1} MiB) | 128 / 256 / 64 LRU / 512 MiB |",
        mib(canvas),
        mib(tex),
        mib(img),
        mib(pinned)
    );
    let s1 = rec.s1();
    let _ = writeln!(
        s,
        "| S1 paint jobs on the asset workers | all | {} jobs, {:.1} kpx, {} failed | damage over 30,000 physical px off the frame thread |",
        s1.jobs,
        s1.pixels as f64 / 1e3,
        s1.failures
    );
    let s7 = rec.s7();
    let _ = writeln!(
        s,
        "| S7 glyph derivation on the frame thread | all | {} glyphs, mean {:.1} µs, max {:.1} µs | ≤ 20 µs per glyph |",
        s7.glyphs, s7.mean_us, s7.max_us
    );
    let _ = writeln!(
        s,
        "| S7 ladder steps on the asset workers | all | {} steps, mean {:.2} ms, max {:.2} ms | ≤ 5 ms per step |",
        s7.steps, s7.step_mean_ms, s7.step_max_ms
    );
    let overall = Summary::of(rec.phase(None).map(|f| f.frame_ms));
    let _ = writeln!(
        s,
        "| Longest frame overall (M3 rule) | all | {:.1} ms ({} frames) | ≤ 50 ms |",
        overall.max, overall.n
    );
    let p = rec.plugins();
    let _ = writeln!(
        s,
        "| Plugin Lua time (reported apart) | all | mean {:.3} ms, p99 {:.3} ms, max {:.3} ms | (U4 frame rule) |",
        p.mean, p.p99, p.max
    );
    let ptr = Summary::of(
        rec.phase(Some("sweep1000"))
            .map(|f| f64::from(f.pointer_events)),
    );
    let _ = writeln!(
        s,
        "| Pointer offers per frame (coalescing) | sweep1000 | mean {:.2}, max {:.0} | ≤ 1 per other event + 1 |",
        ptr.mean, ptr.max
    );
    for (k, v) in notes {
        let _ = writeln!(s, "| {k} | | {v} | |");
    }
    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "| Phase | Frames | U1 mean (all frames) | step 1 / step 7 / step 8 mean (host, plugins incl.) | Plugin mean | Frame mean | Frame max |"
    );
    let _ = writeln!(s, "|---|---|---|---|---|---|---|");
    for ph in rec.phases() {
        let m = |f: fn(&FrameSample) -> f64| Summary::of(rec.phase(Some(ph)).map(f)).mean;
        let u = Summary::of(rec.phase(Some(ph)).map(|f| f.ui_ms));
        let fr = Summary::of(rec.phase(Some(ph)).map(|f| f.frame_ms));
        let _ = writeln!(
            s,
            "| {ph} | {} | {:.3} ms | {:.3} / {:.3} / {:.3} ms | {:.3} ms | {:.2} ms | {:.2} ms |",
            u.n,
            u.mean,
            m(|f| f.step1_ms),
            m(|f| f.step7_ms),
            m(|f| f.paint_ms),
            m(|f| f.plugin_ms),
            fr.mean,
            fr.max
        );
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summaries_and_classes() {
        let s = Summary::of([1.0, 2.0, 3.0, f64::NAN]);
        assert_eq!(s.n, 3);
        assert!((s.mean - 2.0).abs() < 1e-9);
        assert_eq!(s.max, 3.0);
        assert_eq!(percentile(&[], 99.0), 0.0);
        let mut r = Recorder::default();
        r.push(
            "walk",
            FrameSample {
                ui_ms: 0.2,
                upload_bytes: 100,
                ..FrameSample::default()
            },
        );
        r.push(
            "walk",
            FrameSample {
                ui_ms: 9.0,
                content_event: true,
                ..FrameSample::default()
            },
        );
        r.push(
            "open",
            FrameSample {
                frame_ms: 30.0,
                paint_ms: 3.0,
                event: true,
                upload_bytes: 5000,
                ..FrameSample::default()
            },
        );
        // A frame that continues the open's jobs is in the window: not steady, and judged
        // for U4 (a)/(b).
        r.push(
            "open",
            FrameSample {
                frame_ms: 57.0,
                paint_ms: 1.0,
                ui_ms: 2.0,
                event_window: true,
                upload_bytes: 7000,
                ..FrameSample::default()
            },
        );
        assert_eq!(r.u1(Some("walk")).n, 1);
        assert_eq!(r.u1(Some("open")).n, 0);
        assert_eq!(r.u3(), (100.0, 7000));
        assert_eq!(r.u4(), (57.0, 3.0, 9.0));
        assert_eq!(r.phases(), vec!["walk", "open"]);
        let md = report_markdown(&r, "t", &[]);
        assert!(md.contains("U4 (c)") && md.contains("not measured"), "{md}");
        assert!(
            md.contains("Longest frame overall (M3 rule) | all | 57.0 ms"),
            "{md}"
        );
        // Not fed by this run: the row does not blame the adapter.
        assert!(
            md.contains("U2 UI pass GPU time") && md.contains("did not feed"),
            "{md}"
        );
        assert!(!md.contains("no timestamp queries"), "{md}");
    }

    /// render.md §6.16 U2: the steady frames' GPU times are summarised; frames without one
    /// and event frames are left out.
    #[test]
    fn u2_from_timestamp_queries() {
        let mut r = Recorder::default();
        for (gpu, event) in [
            (Some(0.1), false),
            (Some(0.3), false),
            (None, false),
            (Some(5.0), true),
        ] {
            r.push(
                "walk",
                FrameSample {
                    ui_gpu_ms: gpu,
                    event,
                    ..FrameSample::default()
                },
            );
        }
        let u2 = r.u2();
        assert_eq!(u2.n, 2);
        assert!((u2.mean - 0.2).abs() < 1e-9);
        assert_eq!(u2.max, 0.3);
        let md = report_markdown(&r, "t", &[]);
        assert!(
            md.contains("| U2 UI pass GPU time (timestamp queries, steady) | all | mean 0.200 ms"),
            "{md}"
        );
    }

    /// `ui-scaling.md` §8 S1 and S7: the paint step's worker and glyph figures are summed
    /// over the run and reported.
    #[test]
    fn s1_and_s7_rows() {
        let mut r = Recorder::default();
        for (jobs, px, glyphs, ns, max, steps, sns, smax) in [
            (
                1u32,
                60_000u64,
                4u32,
                40_000u64,
                15_000u64,
                2u32,
                6_000_000u64,
                4_000_000u64,
            ),
            (0, 0, 0, 0, 0, 1, 1_000_000, 1_000_000),
        ] {
            r.push(
                "walk",
                FrameSample {
                    worker: WorkerSample {
                        jobs_started: jobs,
                        pixels: px,
                        ..WorkerSample::default()
                    },
                    glyphs: GlyphSample {
                        derived: glyphs,
                        derivation_ns: ns,
                        derivation_max_ns: max,
                        ladder_steps: steps,
                        ladder_step_ns: sns,
                        ladder_step_max_ns: smax,
                        ..GlyphSample::default()
                    },
                    ..FrameSample::default()
                },
            );
        }
        assert_eq!(
            r.s1(),
            S1Summary {
                jobs: 1,
                pixels: 60_000,
                failures: 0
            }
        );
        let s7 = r.s7();
        assert_eq!((s7.glyphs, s7.steps), (4, 3));
        assert!((s7.mean_us - 10.0).abs() < 1e-9);
        assert!((s7.max_us - 15.0).abs() < 1e-9);
        assert!((s7.step_mean_ms - 7.0 / 3.0).abs() < 1e-9);
        assert!((s7.step_max_ms - 4.0).abs() < 1e-9);
        let md = report_markdown(&r, "t", &[]);
        assert!(
            md.contains("| S1 paint jobs on the asset workers | all | 1 jobs, 60.0 kpx, 0 failed"),
            "{md}"
        );
        assert!(md.contains("4 glyphs, mean 10.0 µs, max 15.0 µs"), "{md}");
        assert!(md.contains("3 steps, mean 2.33 ms, max 4.00 ms"), "{md}");
        // Nothing derived: zero means, no division by zero.
        let empty = Recorder::default().s7();
        assert_eq!((empty.mean_us, empty.step_mean_ms), (0.0, 0.0));
    }

    /// A run that fed the renderer's figures but got no GPU time names the adapter (or an
    /// unresolved query) as the reason; one that never fed them says so instead.
    #[test]
    fn u2_not_measured_reason() {
        let mut r = Recorder::default();
        r.push("walk", FrameSample::default());
        let md = report_markdown(&r, "t", &[]);
        assert!(
            md.contains("did not feed the renderer's frame figures"),
            "{md}"
        );
        r.render_stats_fed = true;
        let md = report_markdown(&r, "t", &[]);
        assert!(
            md.contains("the adapter has no timestamp queries, or none resolved"),
            "{md}"
        );
        assert!(!md.contains("did not feed"), "{md}");
    }
}
