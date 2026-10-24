// SPDX-License-Identifier: GPL-3.0-or-later
//! The timing report's figures, its Markdown and its reader ([`TimingReport::parse`]), and
//! the budgets of decision 0043 §3 item 3 ([`TimingReport::budget_failures`]). This file
//! depends on `std` only, so that the fuzz target `client_timing_report` can include it by
//! path; building a report from a run is the parent module's work.

use std::fmt::Write as _;

/// The recorder phase of the frames before the timed window starts.
pub const SETTLE_PHASE: &str = "settle";
/// The recorder phase of the timed window's frames.
pub const TIMED_PHASE: &str = "timed";
/// The frame budget of a 60 Hz frame, milliseconds (M4.5 criterion 5, decision 0043 §3 (b)).
pub const FRAME_BUDGET_MS: f64 = 16.7;
/// The stall limit, milliseconds (M3 criterion 2, decision 0043 §3 (a)).
pub const STALL_MS: f64 = 50.0;
/// The first line of the machine-readable block.
pub const FIGURES_HEADER: &str = "tessera-timing-report 1";
/// Most `key=value` lines [`TimingReport::parse`] reads.
pub const MAX_FIGURE_LINES: usize = 256;
/// Longest input [`TimingReport::parse`] accepts, bytes.
pub const MAX_REPORT_BYTES: usize = 1 << 20;

/// U1 and U2 budgets (decision 0015 §6), milliseconds.
const U1_MEAN_MS: f64 = 0.3;
const U1_P99_MS: f64 = 1.0;
const U2_MS: f64 = 0.3;
/// U3 budgets, bytes.
const U3_MEAN_BYTES: f64 = 256.0 * 1024.0;
const U3_CAP_BYTES: u64 = 8 << 20;
/// U4 (a) frame rule and the (b)/(f) slice, milliseconds.
const U4_FRAME_MS: f64 = 50.0;
const U4_SLICE_MS: f64 = 4.0;
/// U5 caps, bytes.
const U5_CANVAS: u64 = 128 << 20;
const U5_TEXTURES: u64 = 256 << 20;
const U5_UNPINNED: u64 = 64 << 20;
const U5_PINNED: u64 = 512 << 20;

/// Why a timing report could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimingParseError {
    /// The text is larger than [`MAX_REPORT_BYTES`].
    TooLarge,
    /// No `tessera-timing-report 1` block.
    NoFigures,
    /// More than [`MAX_FIGURE_LINES`] lines in the block, or no closing fence.
    Unclosed,
    /// A line that is not `key=value`, a value that does not parse, or a missing key.
    Figure(String),
}

// Written out rather than derived with `thiserror`, so that `fuzz/` can include this file by
// path without the client's dependencies.
impl std::fmt::Display for TimingParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge => write!(f, "timing report larger than {MAX_REPORT_BYTES} bytes"),
            Self::NoFigures => write!(f, "no `{FIGURES_HEADER}` block"),
            Self::Unclosed => write!(
                f,
                "the figures block is not closed within {MAX_FIGURE_LINES} lines"
            ),
            Self::Figure(k) => write!(f, "figure `{k}` is missing or not valid"),
        }
    }
}

impl std::error::Error for TimingParseError {}

/// Windowed or offscreen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    /// A window, presented by the compositor (the run decision 0043 §3 item 2 measures).
    Windowed,
    /// Offscreen, headless.
    Headless,
}

impl RunMode {
    /// The word the report uses (`windowed`, `headless`).
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::Windowed => "windowed",
            Self::Headless => "headless",
        }
    }
}

/// What the scenario feed was doing when the timed window ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedAtEnd {
    /// Still playing: the whole window ran under the scenario's load.
    Running,
    /// Finished before the window ended.
    Finished,
    /// Stopped by a run-time error.
    Failed,
    /// No scenario feed (not a scenario run, or not the sandbox).
    None,
}

impl FeedAtEnd {
    /// The word the report uses (`running`, `finished`, `failed`, `none`).
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Finished => "finished",
            Self::Failed => "failed",
            Self::None => "none",
        }
    }

    fn from_key(s: &str) -> Option<Self> {
        Some(match s {
            "running" => Self::Running,
            "finished" => Self::Finished,
            "failed" => Self::Failed,
            "none" => Self::None,
            _ => return None,
        })
    }
}

/// Where the timed window stood when the report was built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowState {
    /// The window never started: the run ended (the scenario finished, the run timed out
    /// or failed) while it still waited for the player to be placed and the area to settle.
    /// No frame of the run is a timed frame.
    NotStarted,
    /// The window started but its duration was not over when the run ended (the scenario
    /// finished first, or the run stopped): the frames cover only part of it.
    Running,
    /// The window ran its whole duration.
    Done,
}

impl WindowState {
    /// The word the report uses (`not-started`, `running`, `done`).
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::NotStarted => "not-started",
            Self::Running => "running",
            Self::Done => "done",
        }
    }

    fn from_key(s: &str) -> Option<Self> {
        Some(match s {
            "not-started" => Self::NotStarted,
            "running" => Self::Running,
            "done" => Self::Done,
            _ => return None,
        })
    }
}

/// Longest audio backend name [`TimingReport::parse`] keeps, characters.
pub const MAX_BACKEND_CHARS: usize = 64;

/// Frame times of the timed window, from the walker's frame log (`frame_ms`: wall time
/// between frame starts, `walker.md` §7).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FrameFigures {
    /// Frames logged.
    pub frames: usize,
    /// Wall seconds covered.
    pub seconds: f64,
    /// Median frame, ms.
    pub p50_ms: f64,
    /// 95th percentile, ms.
    pub p95_ms: f64,
    /// 99th percentile, ms.
    pub p99_ms: f64,
    /// Longest frame, ms.
    pub max_ms: f64,
    /// Frames over [`FRAME_BUDGET_MS`].
    pub over_frame_budget: usize,
    /// Frames over [`STALL_MS`].
    pub over_stall: usize,
    /// Frames of the window the log could not keep (it was full,
    /// `crate::walker::framelog::MAX_ROWS`): the figures above cover only the first part of
    /// the window when this is not 0.
    pub dropped: usize,
}

/// Audio during the timed window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioFigures {
    /// The running backend, as one lower-case word (`device`, `null`, `offline`,
    /// `disabled`; at most [`MAX_BACKEND_CHARS`] printable ASCII characters). Budget (d) is
    /// measured only with `device`.
    pub backend: String,
    /// Device underruns during the window (0 for backends without a device).
    pub underruns: u64,
}

/// Count, mean, 99th percentile and maximum of a set of values (milliseconds).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Stat {
    /// Number of values.
    pub n: usize,
    /// Mean.
    pub mean: f64,
    /// 99th percentile.
    pub p99: f64,
    /// Maximum.
    pub max: f64,
}

/// The game UI's figures over the timed window (decision 0015 §6, split as decision 0043
/// §3 item 3 (c)).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct UiFigures {
    /// Frames recorded in the timed phase.
    pub frames: usize,
    /// The UI's effective scale *Se* at the end of the run (`ui-scaling.md` §4.3). Budget
    /// (c) holds only at *Se* = 1 (decision 0043 §3 item 3 (c)).
    pub ui_scale: f64,
    /// Whether *Se* was below the user's scale because the surface is under the design floor
    /// of 1024 × 768 (`ui-scaling.md` §4.3).
    pub ui_scale_limited: bool,
    /// Frames of the run the UI recorder could not keep (it was full,
    /// `crate::ui_host::stats::MAX_RECORDED_FRAMES`): the U1–U5 figures cover only the
    /// frames before it filled when this is not 0.
    pub dropped: usize,
    /// U1 over the steady frames: `n`, mean, p99, max (ms).
    pub u1: Stat,
    /// Where the steady U1 time goes: the means of the host's step 1, step 7 and step 8
    /// (paint) over the steady frames, ms (steps 1 and 7 with plugin time, as the scripted
    /// run's phase table reports them).
    pub u1_steps_ms: [f64; 3],
    /// U2 over the steady frames that carry a GPU time; `n` = 0 when not measured.
    pub u2: Stat,
    /// Whether the run fed the renderer's frame figures to the UI host (U2 can be measured
    /// only then).
    pub u2_fed: bool,
    /// U3: steady-state mean bytes.
    pub u3_mean_bytes: f64,
    /// U3: largest upload of any frame, bytes.
    pub u3_max_bytes: u64,
    /// Frames in event windows (U4 (a), (b)).
    pub event_frames: usize,
    /// U4 (a): longest frame in an event window, ms.
    pub u4a_ms: f64,
    /// U4 (b): longest UI paint in an event window, ms.
    pub u4b_ms: f64,
    /// Content-event frames (U4 (f)).
    pub content_frames: usize,
    /// U4 (f): longest UI time in a content-event frame, ms.
    pub u4f_ms: f64,
    /// U5 (over every recorded frame of the run, the settle phase included): peak CPU
    /// canvas bytes.
    pub canvas_bytes: u64,
    /// U5: peak GPU surface texture bytes.
    pub texture_bytes: u64,
    /// U5: peak decoded-image bytes not pinned (the LRU part).
    pub unpinned_bytes: u64,
    /// U5: peak pinned decoded-image bytes.
    pub pinned_bytes: u64,
}

/// The timing report of one run (module docs).
#[derive(Debug, Clone, PartialEq)]
pub struct TimingReport {
    /// A one-line description: surface, mode, adapter.
    pub title: String,
    /// Windowed or headless.
    pub mode: RunMode,
    /// The surface the frames were drawn to (windowed: the size the compositor gave).
    pub surface: (u32, u32),
    /// Paced (vsync, or the headless 60 Hz pacing) or not.
    pub paced: bool,
    /// Where the timed window stood at the end of the run.
    pub window: WindowState,
    /// The area had settled when the window started (`false`: the settle timeout passed, or
    /// the window never started).
    pub settled: bool,
    /// The scenario feed at the end of the window.
    pub feed: FeedAtEnd,
    /// Frame times.
    pub frames: FrameFigures,
    /// Audio, when a backend ran.
    pub audio: Option<AudioFigures>,
    /// The game UI's figures, when the UI recorded the run.
    pub ui: Option<UiFigures>,
}

fn ms(v: f64) -> String {
    format!("{v:.3}")
}

fn mib(b: u64) -> f64 {
    b as f64 / (1024.0 * 1024.0)
}

impl TimingReport {
    /// The report as Markdown, ending with the machine-readable figures block.
    #[must_use]
    pub fn markdown(&self) -> String {
        let mut s = String::new();
        let f = &self.frames;
        let _ = writeln!(s, "### {}\n", self.title);
        let _ = writeln!(s, "| Measure | Value | Budget (decision 0043 §3) |");
        let _ = writeln!(s, "|---|---|---|");
        let _ = writeln!(
            s,
            "| Run | {}, surface {} × {}, {} | — |",
            self.mode.key(),
            self.surface.0,
            self.surface.1,
            if self.paced { "paced" } else { "unpaced" }
        );
        let truncated = if f.dropped > 0 {
            format!(
                "; **the frame log was full: {} frames not logged**",
                f.dropped
            )
        } else {
            String::new()
        };
        match self.window {
            WindowState::NotStarted => {
                let _ = writeln!(
                    s,
                    "| Timed window | **never started**: the run ended while it waited for the player to be placed and the area to settle; scenario at the end: {} | ≥ 60 s with the scenario running |",
                    self.feed.key()
                );
            }
            WindowState::Running | WindowState::Done => {
                let _ = writeln!(
                    s,
                    "| Timed window | {:.1} s, {} frames{}; area settled at the start: {}; scenario at the end: {}{truncated} | ≥ 60 s with the scenario running |",
                    f.seconds,
                    f.frames,
                    if self.window == WindowState::Running {
                        " (**ended early**: the run stopped before the duration was over)"
                    } else {
                        ""
                    },
                    if self.settled {
                        "yes"
                    } else {
                        "no (settle timeout)"
                    },
                    self.feed.key()
                );
            }
        }
        let _ = writeln!(
            s,
            "| Frame time | p50 {:.2} ms, p95 {:.2} ms, p99 {:.2} ms, max {:.1} ms | (b) unpaced: p99 ≤ 16.7 ms |",
            f.p50_ms, f.p95_ms, f.p99_ms, f.max_ms
        );
        let _ = writeln!(s, "| Frames over 16.7 ms | {} | — |", f.over_frame_budget);
        let _ = writeln!(s, "| Frames over 50 ms | {} | (a) paced: 0 |", f.over_stall);
        match &self.audio {
            Some(a) => {
                let _ = writeln!(
                    s,
                    "| Audio underruns | {} (backend {}) | (d) 0 when audio plays |",
                    a.underruns, a.backend
                );
            }
            None => {
                let _ = writeln!(s, "| Audio underruns | no audio | (d) 0 when audio plays |");
            }
        }
        if let Some(u) = &self.ui {
            let _ = writeln!(
                s,
                "| UI scale *Se* | {:.4}{} | 1 (decision 0043 §3 item 3 (c)) |",
                u.ui_scale,
                if u.ui_scale_limited {
                    " (limited by the surface: under 1024 × 768)"
                } else {
                    ""
                }
            );
            if u.dropped > 0 {
                let _ = writeln!(
                    s,
                    "| UI recorder | **full: {} frames not recorded**; U1–U5 cover only the frames before it filled | — |",
                    u.dropped
                );
            }
            let _ = writeln!(
                s,
                "| U1 UI main-thread time (steady) | mean {:.3} ms, p99 {:.3} ms, max {:.3} ms ({} frames of {}) | mean ≤ 0.3 ms, p99 ≤ 1.0 ms |",
                u.u1.mean, u.u1.p99, u.u1.max, u.u1.n, u.frames
            );
            let _ = writeln!(
                s,
                "| U1 by step (steady means) | step 1 {:.3} ms, step 7 {:.3} ms, step 8 {:.3} ms (steps 1 and 7 with plugin time) | — |",
                u.u1_steps_ms[0], u.u1_steps_ms[1], u.u1_steps_ms[2]
            );
            if u.u2.n > 0 {
                let _ = writeln!(
                    s,
                    "| U2 UI pass GPU time (steady) | mean {:.4} ms, p99 {:.4} ms, max {:.4} ms ({} frames) | ≤ 0.3 ms (set for 3840 × 2160) |",
                    u.u2.mean, u.u2.p99, u.u2.max, u.u2.n
                );
            } else {
                let why = if u.u2_fed {
                    "the adapter has no timestamp queries, or none resolved"
                } else {
                    "the run did not feed the renderer's figures to the UI"
                };
                let _ = writeln!(
                    s,
                    "| U2 UI pass GPU time (steady) | not measured ({why}) | not a gate when not measured |"
                );
            }
            let _ = writeln!(
                s,
                "| U3 UI upload bytes | steady mean {:.1} KiB, max {:.1} KiB | mean ≤ 256 KiB, cap 8 MiB |",
                u.u3_mean_bytes / 1024.0,
                u.u3_max_bytes as f64 / 1024.0
            );
            if u.event_frames > 0 {
                let _ = writeln!(
                    s,
                    "| U4 (a) longest frame in an event window | {:.1} ms ({} event frames) | ≤ 50 ms |",
                    u.u4a_ms, u.event_frames
                );
                let _ = writeln!(
                    s,
                    "| U4 (b) longest UI paint in an event window | {:.2} ms | ≤ 4 ms |",
                    u.u4b_ms
                );
            } else {
                let _ = writeln!(s, "| U4 (a), (b) event windows | no event frames | — |");
            }
            let _ = writeln!(
                s,
                "| U4 (c)–(e) | not applicable to this run (decision 0043 §3 item 3 (c)) | — |"
            );
            if u.content_frames > 0 {
                let _ = writeln!(
                    s,
                    "| U4 (f) longest UI time in a content-event frame | {:.2} ms ({} content-event frames) | ≤ 4 ms |",
                    u.u4f_ms, u.content_frames
                );
            } else {
                let _ = writeln!(s, "| U4 (f) content-event frames | none | — |");
            }
            let _ = writeln!(
                s,
                "| U5 memory (peak over the whole run) | canvases {:.1} MiB, textures {:.1} MiB, images {:.1} MiB unpinned, {:.1} MiB pinned | 128 / 256 / 64 / 512 MiB |",
                mib(u.canvas_bytes),
                mib(u.texture_bytes),
                mib(u.unpinned_bytes),
                mib(u.pinned_bytes)
            );
        } else {
            let _ = writeln!(s, "| U1–U5 | no game UI recorded | — |");
        }
        let _ = writeln!(s, "\n```text\n{}", self.figures());
        let _ = writeln!(s, "```");
        s
    }

    /// The `key=value` figures (the block of [`Self::markdown`], without its fences).
    #[must_use]
    pub fn figures(&self) -> String {
        let f = &self.frames;
        let mut s = String::new();
        let mut kv = |k: &str, v: String| {
            let _ = writeln!(s, "{k}={v}");
        };
        kv("mode", self.mode.key().into());
        kv("surface_w", self.surface.0.to_string());
        kv("surface_h", self.surface.1.to_string());
        kv("paced", u8::from(self.paced).to_string());
        kv("window", self.window.key().into());
        kv("settled", u8::from(self.settled).to_string());
        kv("feed", self.feed.key().into());
        kv("frames", f.frames.to_string());
        kv("seconds", format!("{:.3}", f.seconds));
        kv("p50_ms", ms(f.p50_ms));
        kv("p95_ms", ms(f.p95_ms));
        kv("p99_ms", ms(f.p99_ms));
        kv("max_ms", ms(f.max_ms));
        kv("over_16_7", f.over_frame_budget.to_string());
        kv("over_50", f.over_stall.to_string());
        kv("log_dropped", f.dropped.to_string());
        match &self.audio {
            Some(a) => {
                kv("audio_backend", a.backend.clone());
                kv("audio_underruns", a.underruns.to_string());
            }
            None => kv("audio_backend", "none".into()),
        }
        if let Some(u) = &self.ui {
            kv("ui_frames", u.frames.to_string());
            kv("ui_scale", format!("{:.4}", u.ui_scale));
            kv("ui_scale_limited", u8::from(u.ui_scale_limited).to_string());
            kv("ui_dropped", u.dropped.to_string());
            kv("u1_n", u.u1.n.to_string());
            kv("u1_mean_ms", ms(u.u1.mean));
            kv("u1_p99_ms", ms(u.u1.p99));
            kv("u1_max_ms", ms(u.u1.max));
            kv("u1_step1_ms", ms(u.u1_steps_ms[0]));
            kv("u1_step7_ms", ms(u.u1_steps_ms[1]));
            kv("u1_step8_ms", ms(u.u1_steps_ms[2]));
            kv("u2_fed", u8::from(u.u2_fed).to_string());
            kv("u2_n", u.u2.n.to_string());
            kv("u2_mean_ms", format!("{:.4}", u.u2.mean));
            kv("u2_p99_ms", format!("{:.4}", u.u2.p99));
            kv("u2_max_ms", format!("{:.4}", u.u2.max));
            kv("u3_mean_bytes", format!("{:.1}", u.u3_mean_bytes));
            kv("u3_max_bytes", u.u3_max_bytes.to_string());
            kv("u4_event_frames", u.event_frames.to_string());
            kv("u4a_ms", ms(u.u4a_ms));
            kv("u4b_ms", ms(u.u4b_ms));
            kv("u4_content_frames", u.content_frames.to_string());
            kv("u4f_ms", ms(u.u4f_ms));
            kv("u5_canvas_bytes", u.canvas_bytes.to_string());
            kv("u5_texture_bytes", u.texture_bytes.to_string());
            kv("u5_unpinned_bytes", u.unpinned_bytes.to_string());
            kv("u5_pinned_bytes", u.pinned_bytes.to_string());
        }
        format!("{FIGURES_HEADER}\n{s}")
    }

    /// Reads the figures block of a report written by [`Self::markdown`] (the title is not
    /// kept: it is the empty string).
    ///
    /// # Errors
    ///
    /// A [`TimingParseError`] when the text is too large, has no figures block, the block
    /// is not closed, or a figure is missing or does not parse.
    pub fn parse(text: &str) -> Result<Self, TimingParseError> {
        if text.len() > MAX_REPORT_BYTES {
            return Err(TimingParseError::TooLarge);
        }
        let mut lines = text.lines().skip_while(|l| l.trim() != FIGURES_HEADER);
        if lines.next().is_none() {
            return Err(TimingParseError::NoFigures);
        }
        let mut map: std::collections::BTreeMap<&str, &str> = std::collections::BTreeMap::new();
        let mut closed = false;
        for line in lines.take(MAX_FIGURE_LINES) {
            let line = line.trim();
            if line.starts_with("```") {
                closed = true;
                break;
            }
            if line.is_empty() {
                continue;
            }
            let (k, v) = line
                .split_once('=')
                .ok_or_else(|| TimingParseError::Figure(line.chars().take(64).collect()))?;
            map.insert(k, v);
        }
        if !closed {
            return Err(TimingParseError::Unclosed);
        }
        let get = |k: &str| map.get(k).copied();
        let num = |k: &str| -> Result<f64, TimingParseError> {
            get(k)
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|v| v.is_finite())
                .ok_or_else(|| TimingParseError::Figure(k.to_owned()))
        };
        let int = |k: &str| -> Result<u64, TimingParseError> {
            get(k)
                .and_then(|v| v.parse::<u64>().ok())
                .ok_or_else(|| TimingParseError::Figure(k.to_owned()))
        };
        let count = |k: &str| -> Result<usize, TimingParseError> {
            int(k).and_then(|v| usize::try_from(v).map_err(|_| TimingParseError::Figure(k.into())))
        };
        let flag = |k: &str| -> Result<bool, TimingParseError> {
            match get(k) {
                Some("1") => Ok(true),
                Some("0") => Ok(false),
                _ => Err(TimingParseError::Figure(k.to_owned())),
            }
        };
        let dim = |k: &str| -> Result<u32, TimingParseError> {
            int(k).and_then(|v| u32::try_from(v).map_err(|_| TimingParseError::Figure(k.into())))
        };
        let mode = match get("mode") {
            Some("windowed") => RunMode::Windowed,
            Some("headless") => RunMode::Headless,
            _ => return Err(TimingParseError::Figure("mode".into())),
        };
        let feed = get("feed")
            .and_then(FeedAtEnd::from_key)
            .ok_or_else(|| TimingParseError::Figure("feed".into()))?;
        let window = get("window")
            .and_then(WindowState::from_key)
            .ok_or_else(|| TimingParseError::Figure("window".into()))?;
        let frames = FrameFigures {
            frames: count("frames")?,
            seconds: num("seconds")?,
            p50_ms: num("p50_ms")?,
            p95_ms: num("p95_ms")?,
            p99_ms: num("p99_ms")?,
            max_ms: num("max_ms")?,
            over_frame_budget: count("over_16_7")?,
            over_stall: count("over_50")?,
            dropped: count("log_dropped")?,
        };
        let audio = match get("audio_backend") {
            None => return Err(TimingParseError::Figure("audio_backend".into())),
            Some("none") => None,
            // One word of printable ASCII, as the client writes it: a value with white space
            // or control characters, or a long one, is refused rather than cut (a cut value
            // could end in white space, or read `none`, and would not read back the same).
            Some(b)
                if !b.is_empty()
                    && b.len() <= MAX_BACKEND_CHARS
                    && b.bytes().all(|c| c.is_ascii_graphic()) =>
            {
                Some(AudioFigures {
                    backend: b.to_owned(),
                    underruns: int("audio_underruns")?,
                })
            }
            Some(_) => return Err(TimingParseError::Figure("audio_backend".into())),
        };
        let ui = if get("ui_frames").is_some() {
            let summary = |p: &str| -> Result<Stat, TimingParseError> {
                Ok(Stat {
                    n: count(&format!("{p}_n"))?,
                    mean: num(&format!("{p}_mean_ms"))?,
                    p99: num(&format!("{p}_p99_ms"))?,
                    max: num(&format!("{p}_max_ms"))?,
                })
            };
            Some(UiFigures {
                frames: count("ui_frames")?,
                ui_scale: num("ui_scale")?,
                ui_scale_limited: flag("ui_scale_limited")?,
                dropped: count("ui_dropped")?,
                u1: summary("u1")?,
                u1_steps_ms: [
                    num("u1_step1_ms")?,
                    num("u1_step7_ms")?,
                    num("u1_step8_ms")?,
                ],
                u2: summary("u2")?,
                u2_fed: flag("u2_fed")?,
                u3_mean_bytes: num("u3_mean_bytes")?,
                u3_max_bytes: int("u3_max_bytes")?,
                event_frames: count("u4_event_frames")?,
                u4a_ms: num("u4a_ms")?,
                u4b_ms: num("u4b_ms")?,
                content_frames: count("u4_content_frames")?,
                u4f_ms: num("u4f_ms")?,
                canvas_bytes: int("u5_canvas_bytes")?,
                texture_bytes: int("u5_texture_bytes")?,
                unpinned_bytes: int("u5_unpinned_bytes")?,
                pinned_bytes: int("u5_pinned_bytes")?,
            })
        } else {
            None
        };
        Ok(Self {
            title: String::new(),
            mode,
            surface: (dim("surface_w")?, dim("surface_h")?),
            paced: flag("paced")?,
            window,
            settled: flag("settled")?,
            feed,
            frames,
            audio,
            ui,
        })
    }

    /// The budgets of decision 0043 §3 item 3 that this run misses or could not measure, in
    /// words (empty: every budget that applies to the run holds and was measured). A paced
    /// run is judged by (a), (c) and (d); an unpaced run by (b) and (d). A window that never
    /// started, or ended before its duration, is a failure of its own, as is a frame log or
    /// UI recorder that was full (the figures then cover only part of the window). (c) is
    /// judged only at *Se* = 1 and (d) only on the audio device backend; otherwise each is
    /// reported as not measured. U2 is judged only when measured; U4 (a), (b) and (f) only
    /// over the frames the run has. The client never calls this: the budgets are asserted
    /// only with `TESSERA_PERF=1` on the idle reference machine (decision 0010).
    #[must_use]
    pub fn budget_failures(&self) -> Vec<String> {
        let mut out = Vec::new();
        let f = &self.frames;
        match self.window {
            WindowState::NotStarted => {
                out.push("no timed window: the run ended before the window started".to_owned());
                return out;
            }
            WindowState::Running => out.push(format!(
                "the timed window ended early, after {:.1} s",
                f.seconds
            )),
            WindowState::Done => {}
        }
        if f.dropped > 0 {
            out.push(format!(
                "the frame log was full: {} frames of the window not logged",
                f.dropped
            ));
        }
        if self.paced {
            if f.over_stall > 0 {
                out.push(format!("(a) {} frames over 50 ms", f.over_stall));
            }
            if let Some(u) = &self.ui {
                if u.dropped > 0 {
                    out.push(format!(
                        "(c) the UI recorder was full: {} frames not recorded",
                        u.dropped
                    ));
                }
                if (u.ui_scale - 1.0).abs() > 1e-4 {
                    out.push(format!(
                        "(c) not judged: Se = {:.4}, not 1{}",
                        u.ui_scale,
                        if u.ui_scale_limited {
                            " (the surface is under 1024 × 768)"
                        } else {
                            ""
                        }
                    ));
                } else if u.u1.n == 0 {
                    out.push("(c) not measured: no steady UI frame in the window".to_owned());
                } else {
                    Self::ui_failures(u, &mut out);
                }
            } else {
                out.push("(c) not measured: the game UI recorded nothing".to_owned());
            }
        } else if f.p99_ms > FRAME_BUDGET_MS {
            out.push(format!("(b) p99 {:.2} ms", f.p99_ms));
        }
        match &self.audio {
            Some(a) if a.underruns > 0 => {
                out.push(format!("(d) {} audio underruns", a.underruns));
            }
            Some(a) if a.backend == "device" => {}
            Some(a) => out.push(format!(
                "(d) not measured: audio ran on the `{}` backend, not the device (build with `--features audio-device`)",
                a.backend
            )),
            None => out.push("(d) not measured: no audio backend ran".to_owned()),
        }
        out
    }

    /// The U1–U5 budgets of (c) at *Se* = 1.
    fn ui_failures(u: &UiFigures, out: &mut Vec<String>) {
        if u.u1.mean > U1_MEAN_MS || u.u1.p99 > U1_P99_MS {
            out.push(format!(
                "(c) U1 mean {:.3} ms / p99 {:.3} ms",
                u.u1.mean, u.u1.p99
            ));
        }
        if u.u2.n > 0 && u.u2.p99 > U2_MS {
            out.push(format!("(c) U2 p99 {:.3} ms", u.u2.p99));
        }
        if u.u3_mean_bytes > U3_MEAN_BYTES || u.u3_max_bytes > U3_CAP_BYTES {
            out.push(format!(
                "(c) U3 mean {:.1} KiB / max {:.1} KiB",
                u.u3_mean_bytes / 1024.0,
                u.u3_max_bytes as f64 / 1024.0
            ));
        }
        if u.event_frames > 0 && (u.u4a_ms > U4_FRAME_MS || u.u4b_ms > U4_SLICE_MS) {
            out.push(format!(
                "(c) U4 (a) {:.1} ms / (b) {:.2} ms",
                u.u4a_ms, u.u4b_ms
            ));
        }
        if u.content_frames > 0 && u.u4f_ms > U4_SLICE_MS {
            out.push(format!("(c) U4 (f) {:.2} ms", u.u4f_ms));
        }
        if u.canvas_bytes > U5_CANVAS
            || u.texture_bytes > U5_TEXTURES
            || u.unpinned_bytes > U5_UNPINNED
            || u.pinned_bytes > U5_PINNED
        {
            out.push("(c) U5 over a memory cap".to_owned());
        }
    }
}
