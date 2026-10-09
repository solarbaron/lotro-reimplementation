// SPDX-License-Identifier: GPL-3.0-or-later
//! Frame timing and residency records: a rolling window for the HUD and, for automated
//! runs, a per-frame log kept in memory (so the frame loop never writes files) and
//! written as CSV at the end, with a summary.

use std::collections::VecDeque;
use std::io::Write as _;
use std::path::Path;
use std::time::{Duration, Instant};

/// Most rows kept (about 4.6 hours at 60 frames per second).
pub const MAX_ROWS: usize = 1 << 20;
/// A frame longer than this counts as a stall (ROADMAP M3 exit criterion 2).
pub const STALL: Duration = Duration::from_millis(50);

/// One frame.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FrameRow {
    /// Seconds since the log started.
    pub t: f64,
    /// Wall time since the previous frame start, ms.
    pub frame_ms: f64,
    /// Main-thread work of this frame (update and draw recording, without present), ms.
    pub cpu_ms: f64,
    /// Streamer update, ms.
    pub stream_ms: f64,
    /// Bytes uploaded to the GPU.
    pub uploaded: u64,
    /// Resident landblocks.
    pub resident_blocks: usize,
    /// Bytes of resident bundles.
    pub resident_bytes: usize,
    /// Landblocks loading.
    pub loading: usize,
    /// Near-ring landblocks not yet resident.
    pub near_missing: usize,
    /// GPU bytes (terrain, parts, textures).
    pub gpu_bytes: u64,
    /// Process resident set size, bytes (0 when unknown).
    pub rss: u64,
    /// Object draws.
    pub draws: usize,
    /// Terrain meshes drawn.
    pub terrain: usize,
    /// Animated characters (entities with an animation instance).
    pub characters: usize,
    /// Characters drawn.
    pub character_draws: usize,
    /// Main thread: the whole character layer (events, figure uploads, animation), ms.
    pub characters_ms: f64,
    /// Main thread: collecting finished sampling jobs (step 6 of ARCHITECTURE §20.3), ms.
    pub anim_collect_ms: f64,
    /// Main thread: advancing animators and submitting sampling jobs, ms.
    pub anim_submit_ms: f64,
    /// Main thread: uploading skinning palettes while recording the frame, ms.
    pub anim_upload_ms: f64,
    /// Worker time of the sampling jobs collected this frame, ms.
    pub anim_worker_ms: f64,
    /// Main thread: world audio update (graph, engine, properties), ms.
    pub audio_ms: f64,
    /// Audible voices (playing or fading).
    pub audio_voices: usize,
    /// Device underruns so far (cumulative).
    pub audio_underruns: u64,
    /// Mixer load of the last mixer call (render time over audio time).
    pub mixer_load: f64,
    /// Viewer position (region-global).
    pub x: f64,
    /// Viewer position.
    pub y: f64,
    /// Viewer position.
    pub z: f64,
    /// The M4.5 systems (`walker.md` §14.10).
    pub fx: FxRow,
}

/// The frame-log columns of the M4.5 systems (`walker.md` §14.10). The renderer's counters
/// (`gpu_*`) are those of the frame's `end_frame`, filled in afterwards
/// ([`FrameLog::set_render_stats`]).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FxRow {
    /// Main thread: environment and light table, ms.
    pub env_ms: f64,
    /// Main thread: the frill ring update, ms.
    pub frills_ms: f64,
    /// Time of day *t* (`environment.md` §11.1).
    pub time_of_day: f64,
    /// The global limit *G* (`walker.md` §11.3), m.
    pub g: f32,
    /// Fog far distance in force, m (0 without fog).
    pub fog_far: f32,
    /// Terrain blocks drawn with texture layers.
    pub terrain_layered: usize,
    /// Ceiling chunks drawn.
    pub ceilings: usize,
    /// Objects drawn (all forms).
    pub objects_drawn: usize,
    /// Objects drawn while fading.
    pub fading: usize,
    /// Part instances drawn instanced (generated scenery).
    pub instanced: usize,
    /// Instanced scenery batches.
    pub batches: usize,
    /// Water quads queued.
    pub water_quads: usize,
    /// Frill groups queued.
    pub frill_groups: usize,
    /// Frill instances queued.
    pub frill_instances: usize,
    /// Lights in the frame's table.
    pub lights: usize,
    /// Renderer: draw calls.
    pub gpu_draw_calls: u32,
    /// Renderer: terrain layer draws.
    pub gpu_terrain_layers: u32,
    /// Renderer: instances drawn (scenery and frills).
    pub gpu_instances: u32,
    /// Renderer: water quads drawn.
    pub gpu_water_quads: u32,
    /// Renderer: sky objects drawn.
    pub gpu_sky_objects: u32,
    /// Renderer: shadow-map draws.
    pub gpu_shadow_draws: u32,
    /// Renderer: lit mesh draws.
    pub gpu_lit_draws: u32,
    /// Main thread: the renderer's `end_frame` (frame building and submission), ms.
    pub end_frame_ms: f64,
    /// Main thread: waiting for the GPU after submission (headless runs), ms.
    pub gpu_wait_ms: f64,
    /// Main-thread phases, ms: terrain update, object update, minimap update, terrain
    /// recording, object recording, water recording, sky recording, frill recording.
    pub phase_ms: [f64; 8],
}

/// Summary of a log.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Summary {
    /// Frames.
    pub frames: usize,
    /// Wall seconds covered.
    pub seconds: f64,
    /// Longest frame, ms.
    pub max_frame_ms: f64,
    /// Median frame, ms.
    pub p50_ms: f64,
    /// 99th percentile frame, ms.
    pub p99_ms: f64,
    /// Longest main-thread work, ms.
    pub max_cpu_ms: f64,
    /// Longest streamer update, ms.
    pub max_stream_ms: f64,
    /// Frames over [`STALL`].
    pub stalls: usize,
    /// Peak resident landblocks.
    pub peak_blocks: usize,
    /// Peak bytes of resident bundles.
    pub peak_resident_bytes: usize,
    /// Peak GPU bytes.
    pub peak_gpu_bytes: u64,
    /// Peak process RSS.
    pub peak_rss: u64,
    /// Frames with near-ring holes.
    pub near_missing_frames: usize,
    /// 95th percentile frame, ms.
    pub p95_ms: f64,
    /// Mean frames per second (frames over the summed frame times).
    pub fps: f64,
    /// Most animated characters in one frame.
    pub max_characters: usize,
    /// Fewest characters drawn in one frame (over frames with any animated character).
    pub min_character_draws: usize,
    /// Mean main-thread animation work per frame (collect, submit, palette upload), ms.
    pub anim_main_ms: f64,
    /// Longest main-thread animation work in one frame, ms.
    pub max_anim_main_ms: f64,
    /// Mean worker time of sampling per frame, ms.
    pub anim_worker_ms: f64,
    /// Longest main-thread time of the whole character layer in one frame, ms.
    pub max_characters_ms: f64,
    /// Mean main-thread audio update per frame, ms.
    pub audio_ms: f64,
    /// Longest main-thread audio update, ms.
    pub max_audio_ms: f64,
    /// Device underruns during the log (last row's count minus the first's).
    pub audio_underruns: u64,
    /// Highest mixer load seen in a row.
    pub max_mixer_load: f64,
    /// Most audible voices in one frame.
    pub max_audio_voices: usize,
}

impl Summary {
    /// Share of the animation work done on workers: worker time over worker plus
    /// main-thread animation time (0 without animation).
    #[must_use]
    pub fn worker_share(&self) -> f64 {
        let total = self.anim_worker_ms + self.anim_main_ms;
        if total > 0.0 {
            self.anim_worker_ms / total
        } else {
            0.0
        }
    }
}

impl std::fmt::Display for Summary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mib = |b: f64| b / f64::from(1 << 20);
        write!(
            f,
            "{} frames in {:.1} s; frame time max {:.1} ms, p50 {:.1} ms, p99 {:.1} ms; \
             main-thread work max {:.1} ms (streamer max {:.2} ms); {} frames over {} ms; \
             peak residency {} landblocks / {:.1} MiB bundles, {:.1} MiB GPU, {:.1} MiB RSS; \
             {} frames with near-ring holes",
            self.frames,
            self.seconds,
            self.max_frame_ms,
            self.p50_ms,
            self.p99_ms,
            self.max_cpu_ms,
            self.max_stream_ms,
            self.stalls,
            STALL.as_millis(),
            self.peak_blocks,
            mib(self.peak_resident_bytes as f64),
            mib(self.peak_gpu_bytes as f64),
            mib(self.peak_rss as f64),
            self.near_missing_frames,
        )?;
        if self.max_characters > 0 {
            write!(
                f,
                "; {:.1} fps mean, p95 {:.2} ms; up to {} animated characters ({} drawn at least); animation main thread {:.3} ms/frame (max {:.2} ms), \
                 workers {:.3} ms/frame: {:.1} % of the animation work on workers; character \
                 layer max {:.2} ms",
                self.fps,
                self.p95_ms,
                self.max_characters,
                self.min_character_draws,
                self.anim_main_ms,
                self.max_anim_main_ms,
                self.anim_worker_ms,
                100.0 * self.worker_share(),
                self.max_characters_ms,
            )?;
        }
        if self.max_audio_ms > 0.0 {
            write!(
                f,
                "; audio {:.3} ms/frame (max {:.2} ms), up to {} voices, {} underruns, mixer load max {:.1} %",
                self.audio_ms,
                self.max_audio_ms,
                self.max_audio_voices,
                self.audio_underruns,
                100.0 * self.max_mixer_load,
            )?;
        }
        Ok(())
    }
}

/// The in-memory log.
#[derive(Debug, Default)]
pub struct FrameLog {
    rows: Vec<FrameRow>,
    dropped: usize,
}

impl FrameLog {
    /// Appends a row (dropped past [`MAX_ROWS`]).
    pub fn push(&mut self, row: FrameRow) {
        if self.rows.len() < MAX_ROWS {
            self.rows.push(row);
        } else {
            self.dropped += 1;
        }
    }

    /// Rows so far.
    #[must_use]
    pub fn rows(&self) -> &[FrameRow] {
        &self.rows
    }

    /// Fills the submission and GPU-wait times of the last row.
    pub fn set_frame_timing(&mut self, end_frame_ms: f64, gpu_wait_ms: f64) {
        if let Some(r) = self.rows.last_mut() {
            r.fx.end_frame_ms = end_frame_ms;
            r.fx.gpu_wait_ms = gpu_wait_ms;
        }
    }

    /// Fills the renderer's counters of the last row (its frame has just ended).
    pub fn set_render_stats(&mut self, stats: &tessera_render::FrameStats) {
        if let Some(r) = self.rows.last_mut() {
            r.fx.gpu_draw_calls = stats.draw_calls;
            r.fx.gpu_terrain_layers = stats.terrain_layers;
            r.fx.gpu_instances = stats.instances;
            r.fx.gpu_water_quads = stats.water_quads;
            r.fx.gpu_sky_objects = stats.sky_objects;
            r.fx.gpu_shadow_draws = stats.shadow_draws;
            r.fx.gpu_lit_draws = stats.lit_draws;
        }
    }

    /// The summary of every row.
    #[must_use]
    pub fn summary(&self) -> Summary {
        summarize(&self.rows)
    }

    /// Writes the rows as CSV (header first).
    ///
    /// # Errors
    ///
    /// I/O errors.
    pub fn write_csv(&self, path: &Path) -> std::io::Result<()> {
        let mut w = std::io::BufWriter::new(std::fs::File::create(path)?);
        // The audio columns (M6) are appended last, after the M4.5 columns that end in
        // `frills_draw_ms`, so that readers of the earlier columns keep working (walker.md
        // §7, "Audio columns").
        writeln!(
            w,
            "frame,t_s,frame_ms,cpu_ms,stream_ms,uploaded_bytes,resident_blocks,resident_bytes,\
             loading,near_missing,gpu_bytes,rss_bytes,object_draws,terrain_draws,characters,\
             character_draws,characters_ms,anim_collect_ms,anim_submit_ms,anim_upload_ms,anim_worker_ms,x,y,z,\
             env_ms,frills_ms,time_of_day,g,fog_far,terrain_layered,ceilings,objects_drawn,fading,\
             instanced,batches,water_quads,frill_groups,frill_instances,lights,gpu_draw_calls,\
             gpu_terrain_layers,gpu_instances,gpu_water_quads,gpu_sky_objects,gpu_shadow_draws,\
             gpu_lit_draws,end_frame_ms,gpu_wait_ms,terrain_update_ms,objects_update_ms,\
             minimap_update_ms,terrain_draw_ms,objects_draw_ms,water_draw_ms,sky_draw_ms,\
             frills_draw_ms,\
             audio_ms,audio_voices,audio_underruns,mixer_load"
        )?;
        for (i, r) in self.rows.iter().enumerate() {
            writeln!(
                w,
                "{i},{:.4},{:.3},{:.3},{:.3},{},{},{},{},{},{},{},{},{},{},{},{:.4},{:.4},{:.4},{:.4},{:.4},\
                 {:.2},{:.2},{:.2},{:.4},{:.4},{:.5},{:.1},{:.1},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},\
                 {:.4},{},{},{:.4}",
                r.t,
                r.frame_ms,
                r.cpu_ms,
                r.stream_ms,
                r.uploaded,
                r.resident_blocks,
                r.resident_bytes,
                r.loading,
                r.near_missing,
                r.gpu_bytes,
                r.rss,
                r.draws,
                r.terrain,
                r.characters,
                r.character_draws,
                r.characters_ms,
                r.anim_collect_ms,
                r.anim_submit_ms,
                r.anim_upload_ms,
                r.anim_worker_ms,
                r.x,
                r.y,
                r.z,
                r.fx.env_ms,
                r.fx.frills_ms,
                r.fx.time_of_day,
                r.fx.g,
                r.fx.fog_far,
                r.fx.terrain_layered,
                r.fx.ceilings,
                r.fx.objects_drawn,
                r.fx.fading,
                r.fx.instanced,
                r.fx.batches,
                r.fx.water_quads,
                r.fx.frill_groups,
                r.fx.frill_instances,
                r.fx.lights,
                r.fx.gpu_draw_calls,
                r.fx.gpu_terrain_layers,
                r.fx.gpu_instances,
                r.fx.gpu_water_quads,
                r.fx.gpu_sky_objects,
                r.fx.gpu_shadow_draws,
                r.fx.gpu_lit_draws,
                r.fx.end_frame_ms,
                r.fx.gpu_wait_ms,
                r.fx.phase_ms[0],
                r.fx.phase_ms[1],
                r.fx.phase_ms[2],
                r.fx.phase_ms[3],
                r.fx.phase_ms[4],
                r.fx.phase_ms[5],
                r.fx.phase_ms[6],
                r.fx.phase_ms[7],
                r.audio_ms,
                r.audio_voices,
                r.audio_underruns,
                r.mixer_load
            )?;
        }
        if self.dropped > 0 {
            tracing::warn!(dropped = self.dropped, "frame log was full");
        }
        w.flush()
    }
}

/// The summary of `rows`.
#[must_use]
pub fn summarize(rows: &[FrameRow]) -> Summary {
    if rows.is_empty() {
        return Summary::default();
    }
    let mut times: Vec<f64> = rows.iter().map(|r| r.frame_ms).collect();
    times.sort_unstable_by(f64::total_cmp);
    let pct = |q: f64| times[((times.len() - 1) as f64 * q).round() as usize];
    let total_ms: f64 = times.iter().sum();
    let n = rows.len() as f64;
    let mut s = Summary {
        frames: rows.len(),
        seconds: rows.last().map_or(0.0, |r| r.t) - rows.first().map_or(0.0, |r| r.t),
        p50_ms: pct(0.5),
        p95_ms: pct(0.95),
        p99_ms: pct(0.99),
        fps: if total_ms > 0.0 {
            n * 1000.0 / total_ms
        } else {
            0.0
        },
        min_character_draws: usize::MAX,
        ..Summary::default()
    };
    for r in rows {
        let main = r.anim_collect_ms + r.anim_submit_ms + r.anim_upload_ms;
        s.anim_main_ms += main / n;
        s.anim_worker_ms += r.anim_worker_ms / n;
        s.max_anim_main_ms = s.max_anim_main_ms.max(main);
        s.max_characters_ms = s.max_characters_ms.max(r.characters_ms);
        s.max_characters = s.max_characters.max(r.characters);
        s.audio_ms += r.audio_ms / n;
        s.max_audio_ms = s.max_audio_ms.max(r.audio_ms);
        s.max_mixer_load = s.max_mixer_load.max(r.mixer_load);
        s.max_audio_voices = s.max_audio_voices.max(r.audio_voices);
        if r.characters > 0 {
            s.min_character_draws = s.min_character_draws.min(r.character_draws);
        }
        s.max_frame_ms = s.max_frame_ms.max(r.frame_ms);
        s.max_cpu_ms = s.max_cpu_ms.max(r.cpu_ms);
        s.max_stream_ms = s.max_stream_ms.max(r.stream_ms);
        s.stalls += usize::from(r.frame_ms > STALL.as_secs_f64() * 1e3);
        s.peak_blocks = s.peak_blocks.max(r.resident_blocks);
        s.peak_resident_bytes = s.peak_resident_bytes.max(r.resident_bytes);
        s.peak_gpu_bytes = s.peak_gpu_bytes.max(r.gpu_bytes);
        s.peak_rss = s.peak_rss.max(r.rss);
        s.near_missing_frames += usize::from(r.near_missing > 0);
    }
    s.audio_underruns = rows
        .last()
        .map_or(0, |r| r.audio_underruns)
        .saturating_sub(rows.first().map_or(0, |r| r.audio_underruns));
    if s.min_character_draws == usize::MAX {
        s.min_character_draws = 0;
    }
    s
}

/// Rolling frame statistics for the HUD: frames per second and the longest frame over
/// the last second.
#[derive(Debug, Default)]
pub struct Rolling {
    window: VecDeque<(Instant, f64)>,
}

impl Rolling {
    /// Records a frame that ended at `now` and took `ms`.
    pub fn push(&mut self, now: Instant, ms: f64) {
        self.window.push_back((now, ms));
        while let Some(&(t, _)) = self.window.front() {
            if now.saturating_duration_since(t) > Duration::from_secs(1)
                || self.window.len() > 10_000
            {
                self.window.pop_front();
            } else {
                break;
            }
        }
    }

    /// (frames per second, mean ms, max ms) over the window.
    #[must_use]
    pub fn stats(&self) -> (f64, f64, f64) {
        let n = self.window.len();
        if n == 0 {
            return (0.0, 0.0, 0.0);
        }
        let sum: f64 = self.window.iter().map(|w| w.1).sum();
        let max = self.window.iter().map(|w| w.1).fold(0.0, f64::max);
        let fps = if sum > 0.0 {
            n as f64 * 1000.0 / sum
        } else {
            0.0
        };
        (fps, sum / n as f64, max)
    }
}

/// This process's resident set size in bytes (Linux; 0 elsewhere or on error). Reads a
/// procfs file: call it at a low rate.
#[must_use]
pub fn rss_bytes() -> u64 {
    let Ok(s) = std::fs::read_to_string("/proc/self/status") else {
        return 0;
    };
    s.lines()
        .find(|l| l.starts_with("VmRSS:"))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|v| v.parse::<u64>().ok())
        .map_or(0, |kb| kb * 1024)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_and_csv() {
        let mut log = FrameLog::default();
        for i in 0..100 {
            log.push(FrameRow {
                t: f64::from(i) / 60.0,
                frame_ms: if i == 50 { 70.0 } else { 16.0 },
                resident_blocks: i as usize,
                gpu_bytes: 5,
                ..FrameRow::default()
            });
        }
        let s = log.summary();
        assert_eq!(s.frames, 100);
        assert_eq!(s.max_frame_ms, 70.0);
        assert_eq!(s.p50_ms, 16.0);
        assert_eq!(s.stalls, 1);
        assert_eq!(s.peak_blocks, 99);
        assert!(s.to_string().contains("1 frames over 50 ms"));
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("log.csv");
        log.write_csv(&p).unwrap();
        let text = std::fs::read_to_string(&p).unwrap();
        assert_eq!(text.lines().count(), 101);
        assert_eq!(summarize(&[]), Summary::default());
        assert_eq!(s.max_characters, 0);
        assert!(!s.to_string().contains("animated"));
    }

    #[test]
    fn animation_summary() {
        let rows: Vec<FrameRow> = (0..10)
            .map(|i| FrameRow {
                t: f64::from(i) / 100.0,
                frame_ms: 10.0,
                characters: 50,
                character_draws: if i == 3 { 49 } else { 50 },
                anim_collect_ms: 0.1,
                anim_submit_ms: 0.2,
                anim_upload_ms: 0.1,
                anim_worker_ms: 1.6,
                ..FrameRow::default()
            })
            .collect();
        let s = summarize(&rows);
        assert!((s.fps - 100.0).abs() < 1e-9);
        assert_eq!(s.p95_ms, 10.0);
        assert_eq!((s.max_characters, s.min_character_draws), (50, 49));
        assert!((s.anim_main_ms - 0.4).abs() < 1e-9);
        assert!((s.worker_share() - 0.8).abs() < 1e-9);
        assert!(
            s.to_string()
                .contains("80.0 % of the animation work on workers")
        );
    }

    #[test]
    fn audio_summary() {
        let rows: Vec<FrameRow> = (0..4)
            .map(|i| FrameRow {
                t: f64::from(i),
                frame_ms: 16.0,
                audio_ms: 0.5,
                audio_voices: i as usize,
                audio_underruns: if i == 3 { 7 } else { 2 },
                mixer_load: 0.1 * f64::from(i),
                ..FrameRow::default()
            })
            .collect();
        let s = summarize(&rows);
        assert_eq!((s.audio_underruns, s.max_audio_voices), (5, 3));
        assert!((s.audio_ms - 0.5).abs() < 1e-12 && (s.max_mixer_load - 0.3).abs() < 1e-12);
        assert!(s.to_string().contains("5 underruns"));
        assert!(!summarize(&rows[..0]).to_string().contains("audio"));
    }

    /// `walker.md` §7 "Audio columns": the four audio columns come last, right after
    /// `frills_draw_ms`, and every row has as many fields as the header.
    #[test]
    fn audio_columns_come_last() {
        let mut log = FrameLog::default();
        log.push(FrameRow {
            audio_ms: 0.25,
            audio_voices: 3,
            audio_underruns: 1,
            mixer_load: 0.5,
            ..FrameRow::default()
        });
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("log.csv");
        log.write_csv(&p).unwrap();
        let text = std::fs::read_to_string(&p).unwrap();
        let mut lines = text.lines();
        let header: Vec<&str> = lines.next().unwrap().split(',').collect();
        let n = header.len();
        assert_eq!(
            header[n - 5..],
            [
                "frills_draw_ms",
                "audio_ms",
                "audio_voices",
                "audio_underruns",
                "mixer_load"
            ]
        );
        let row: Vec<&str> = lines.next().unwrap().split(',').collect();
        assert_eq!(row.len(), n);
        assert_eq!(row[n - 4..], ["0.2500", "3", "1", "0.5000"]);
    }

    #[test]
    fn rolling_window() {
        let mut r = Rolling::default();
        let t = Instant::now();
        for i in 0..10 {
            r.push(t + Duration::from_millis(i * 10), 10.0);
        }
        let (fps, mean, max) = r.stats();
        assert!((fps - 100.0).abs() < 1e-6 && mean == 10.0 && max == 10.0);
    }
}
