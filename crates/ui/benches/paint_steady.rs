// SPDX-License-Identifier: GPL-3.0-or-later
//! Steady-state benchmark of frame-loop step 8 (`Ui::paint`) on a synthetic UI shaped like
//! the P8 busiest scenario (`docs/perf/p8-busiest.md`, section "ui-perf"): static panels
//! (bags, a character panel, a quest tracker) and a HUD that changes every frame (a
//! transparent layer window with floating texts that rise, grow and fade, vital and cast
//! bars, cooldown labels, a chat window with ten lines a second). Synthetic images and
//! Tessera's bundled font; 1920 × 1080 under a UI screen at *Se* = 1.
//!
//! Run with `cargo bench -p tessera-ui --bench paint_steady` (a release build). Options:
//!
//! - `--frames N`: frames per phase (default 600).
//! - `--check`: paints every frame on a second UI with the reference paint step
//!   (`Ui::set_reference_paint`: the full surface pass every frame) and requires equal
//!   composition lists and GPU textures (a CPU model of the `GpuOp`s) after every frame.
//! - `--assert`: exits non-zero when a frame without a change runs a surface pass, starts a
//!   job, paints a band or uploads anything, or when the HUD phases run the full pass on more
//!   than 5 % of their frames (the regression the "ui-perf" section records).
//!
//! Phases: `idle` (nothing changes), `hud` (the HUD changes every frame, the panels are
//! open), `hud-alone` (the same with the panels closed: the panels' share of a steady
//! frame is the difference), `floaters` (only the floating texts rise, grow and fade, with
//! no new text: the geometry preview of `ui-scaling.md` §6.1 and `render.md` §6.12 R13 make
//! that free of paint), `rest` (nothing changes after it: the floating texts get their one
//! settle repaint, §6.1 item 4). With `--assert` the `floaters` phase must start no job,
//! paint no band and upload nothing, and after `rest` no geometry preview may be pending and
//! painting must have settled. Times are the paint call's **thread CPU time**
//! (`CLOCK_THREAD_CPUTIME_ID`); results are **indicative** unless taken on the reference
//! machine under rule 1 of `docs/perf/reference-machine.md`.

mod common;

use std::collections::HashMap;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_formats::Did;
use tessera_formats::ui_layout::UiLayout;
use tessera_material::ui_image::{PixelLayout, UiImage};
use tessera_ui::paint::{GpuOp, Rect, Slot, TexKey, TimeSlice, Unlimited};
use tessera_ui::scaling::{UiScreen, safe_area};
use tessera_ui::text::{
    CharListBuilder, DEFAULT_RANGES, FontFace, LayoutParams, TesseraFont, layout,
};
use tessera_ui::{
    Background, ControlKind, ElementKey, ImageRef, ImageState, PreparedText, TextRequest, Ui,
    UiEnv, Vocab,
};

const SCREEN: (i32, i32) = (1920, 1080);
/// The paint slice of every timed frame (UR §6 U4 (b)).
const SLICE: Duration = Duration::from_millis(4);
const IMG_PANEL: u32 = 0x4100_0001;
const IMG_ICON: u32 = 0x4100_0002;
const IMG_FRAME: u32 = 0x4100_0003;

/// Synthetic images and the bundled fallback font.
struct BenchEnv {
    images: HashMap<ImageRef, Arc<UiImage>>,
    face: Arc<FontFace>,
}

impl UiEnv for BenchEnv {
    fn layout(&mut self, _did: Did) -> Option<Arc<UiLayout>> {
        None
    }

    fn image(&mut self, image: &ImageRef) -> ImageState {
        match self.images.get(image) {
            Some(i) => ImageState::Ready(Arc::clone(i)),
            None => ImageState::Failed,
        }
    }

    fn shape_text(&mut self, request: &TextRequest<'_>) -> Option<Arc<PreparedText>> {
        let fonts = vec![Some(Arc::clone(&self.face))];
        let mut b = CharListBuilder::new();
        b.colour(0xFFF0_E0C0).push_str(request.text);
        let chars = b.finish();
        let p = LayoutParams {
            width: request.size.0,
            height: request.size.1,
            ..LayoutParams::default()
        };
        let l = layout(&chars, &fonts, &p, None).ok()?;
        Some(Arc::new(PreparedText {
            layout: l,
            chars,
            fonts,
            outline: true,
            outline_colour: 0xFF00_0000,
            truncated: false,
            extent: (request.size.0, request.size.1),
        }))
    }
}

fn noise(w: u32, h: u32, seed: u32, alpha: u32) -> Arc<UiImage> {
    let mut x = seed.wrapping_mul(2_654_435_761).max(1);
    let px = (0..w * h)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            (alpha << 24) | (x & 0xFF_FFFF)
        })
        .collect();
    Arc::new(UiImage::new(w, h, w, h, PixelLayout::A8R8G8B8, px).expect("valid image"))
}

fn control(ui: &mut Ui, kind: ControlKind, parent: ElementKey, b: [i32; 4]) -> ElementKey {
    let k = ui.create_control(kind, 1).expect("control");
    ui.control_set_parent(k, Some(parent)).expect("parent");
    ui.control_set_position(k, b[0], b[1]);
    ui.control_set_size(k, b[2], b[3]);
    ui.control_set_visible(k, true);
    k
}

fn window(ui: &mut Ui, b: [i32; 4]) -> ElementKey {
    let k = ui.create_control(ControlKind::Window, 1).expect("window");
    ui.control_set_position(k, b[0], b[1]);
    ui.control_set_size(k, b[2], b[3]);
    ui.control_set_visible(k, true);
    k
}

fn label(ui: &mut Ui, parent: ElementKey, b: [i32; 4], text: &str) -> ElementKey {
    let k = control(ui, ControlKind::Label, parent, b);
    ui.set_text(k, Some(text.to_string()));
    k
}

struct Scene {
    ui: Ui,
    panels: Vec<ElementKey>,
    floaters: Vec<ElementKey>,
    bars: Vec<ElementKey>,
    timers: Vec<ElementKey>,
    chat: Vec<ElementKey>,
}

fn scene(reference: bool) -> Scene {
    let font = TesseraFont::bundled().expect("bundled font");
    let face = Arc::new(font.rasterise(14, DEFAULT_RANGES).expect("rasterise"));
    let mut images = HashMap::new();
    images.insert(ImageRef::Did(IMG_PANEL), noise(64, 64, 1, 0xC0));
    images.insert(ImageRef::Did(IMG_ICON), noise(32, 32, 2, 0xFF));
    images.insert(ImageRef::Did(IMG_FRAME), noise(40, 40, 3, 0x60));
    let mut ui = Ui::new(
        Box::new(BenchEnv { images, face }),
        Vocab::default(),
        SCREEN,
    );
    let f = UiScreen::new(SCREEN.0, SCREEN.1, 1.0).expect("screen");
    ui.set_ui_screen(f, safe_area(&f, None));
    ui.set_reference_paint(reference);
    let mut panels = Vec::new();
    // Bags: six bags of 6 × 4 slots, each with icon, quality frame, cooldown shade, count
    // label and an empty highlight.
    for b in 0..6 {
        let w = window(
            &mut ui,
            [1000 + (b % 3) * 310, 420 + (b / 3) * 260, 300, 250],
        );
        ui.control_set_background(w, Background::Did(IMG_PANEL));
        label(&mut ui, w, [8, 4, 200, 18], &format!("Bag {}", b + 1));
        for s in 0..24 {
            let slot = control(
                &mut ui,
                ControlKind::Control,
                w,
                [10 + (s % 6) * 48, 30 + (s / 6) * 52, 44, 44],
            );
            ui.control_set_background(slot, Background::Did(IMG_ICON));
            let fr = control(&mut ui, ControlKind::Control, slot, [0, 0, 44, 44]);
            ui.control_set_background(fr, Background::Did(IMG_FRAME));
            let shade = control(&mut ui, ControlKind::Control, slot, [0, 30, 44, 14]);
            ui.control_set_back_colour(shade, Some([0.0, 0.0, 0.0, 0.3]));
            control(&mut ui, ControlKind::Control, slot, [2, 2, 40, 40]);
            label(
                &mut ui,
                slot,
                [24, 28, 20, 16],
                &format!("{}", (s * 7) % 50),
            );
        }
        panels.push(w);
    }
    // The character panel: 20 equipment slots and 24 stat rows.
    let cp = window(&mut ui, [262, 118, 586, 576]);
    ui.control_set_background(cp, Background::Did(IMG_PANEL));
    for s in 0..20 {
        let slot = control(
            &mut ui,
            ControlKind::Control,
            cp,
            [20 + (s % 2) * 480, 40 + (s / 2) * 50, 44, 44],
        );
        ui.control_set_background(slot, Background::Did(IMG_ICON));
    }
    for r in 0..24 {
        label(
            &mut ui,
            cp,
            [100, 40 + r * 20, 180, 18],
            &format!("Stat {r}"),
        );
        label(
            &mut ui,
            cp,
            [300, 40 + r * 20, 80, 18],
            &format!("{}", r * 13),
        );
    }
    panels.push(cp);
    // The quest tracker: twelve lines.
    let tr = window(&mut ui, [14, 121, 220, 451]);
    for l in 0..12 {
        label(
            &mut ui,
            tr,
            [4, 4 + l * 22, 212, 20],
            &format!("Objective {l}: 0/5"),
        );
    }
    panels.push(tr);
    // The HUD layer: a transparent screen-sized window with floating texts (canvas resize
    // mode, own surfaces nested in the layer).
    let layer = window(&mut ui, [0, 0, SCREEN.0, SCREEN.1]);
    let floaters = (0..4)
        .map(|i| {
            let k = label(&mut ui, layer, [500 + i * 200, 400, 160, 40], "-85");
            ui.control_set_stretch_mode(k, 3);
            k
        })
        .collect();
    // Unit frames: vital and cast bars (backgrounds whose width changes) and timers.
    let mut bars = Vec::new();
    let mut timers = Vec::new();
    for u in 0..7 {
        let w = window(&mut ui, [20 + (u % 4) * 330, 20 + (u / 4) * 90, 319, 80]);
        ui.control_set_background(w, Background::Did(IMG_PANEL));
        label(&mut ui, w, [8, 4, 200, 18], &format!("Unit {u}"));
        for b in 0..2 {
            let bar = control(&mut ui, ControlKind::Control, w, [8, 26 + b * 18, 300, 14]);
            ui.control_set_back_colour(bar, Some([0.8, 0.2 * b as f32, 0.1, 1.0]));
            bars.push(bar);
        }
        timers.push(label(&mut ui, w, [250, 4, 60, 18], "10s"));
    }
    // A cast bar.
    let cast = window(&mut ui, [830, 812, 259, 65]);
    let fill = control(&mut ui, ControlKind::Control, cast, [2, 28, 254, 25]);
    ui.control_set_back_colour(fill, Some([0.9, 0.7, 0.2, 1.0]));
    bars.push(fill);
    timers.push(label(&mut ui, cast, [2, 4, 254, 20], "Rallying cry"));
    // The chat window: twelve lines.
    let cw = window(&mut ui, [0, 760, 640, 300]);
    ui.control_set_background(cw, Background::Did(IMG_PANEL));
    let chat = (0..12)
        .map(|l| label(&mut ui, cw, [8, 8 + l * 22, 620, 20], &format!("line {l}")))
        .collect();
    Scene {
        ui,
        panels,
        floaters,
        bars,
        timers,
        chat,
    }
}

/// The floating texts' changes of frame `f`: each rises and grows for 90 frames, then starts
/// again lower down (with a new text when `text`).
fn float_step(s: &mut Scene, f: u32, text: bool) {
    for (i, k) in s.floaters.clone().into_iter().enumerate() {
        let age = (f + i as u32 * 23) % 90;
        let (x, _) = s.ui.control_position(k);
        s.ui.control_set_position(k, x, 420 - age as i32);
        let scale = 1.0 + age as f32 / 120.0;
        s.ui.set_node_scale(k, Some([scale, scale, 1.0]));
        s.ui.control_set_opacity(k, 1.0 - age as f32 / 120.0);
        if text && age == 0 {
            s.ui.set_text(k, Some(format!("-{}", 60 + (f % 40))));
        }
    }
}

/// The HUD's changes of frame `f` (60 frames a second).
fn hud_step(s: &mut Scene, f: u32) {
    float_step(s, f, true);
    for (i, k) in s.bars.clone().into_iter().enumerate() {
        // Vitals change every 90 frames, the cast bar every frame.
        if i + 1 == s.bars.len() || (f + i as u32 * 7).is_multiple_of(90) {
            let w = 40 + ((f + i as u32 * 31) % 260) as i32;
            let (_, h) = s.ui.control_size(k);
            s.ui.control_set_size(k, w, h);
        }
    }
    if f.is_multiple_of(60) {
        for (i, k) in s.timers.clone().into_iter().enumerate() {
            s.ui.set_text(k, Some(format!("{}s", (f / 60 + i as u32) % 30)));
        }
    }
    // Ten chat lines a second: three lines every 18 frames.
    if f.is_multiple_of(18) {
        let n = s.chat.len();
        for (i, k) in s.chat.clone().into_iter().enumerate() {
            let line = (f / 18) as usize * 3 + i;
            if i + 3 >= n {
                s.ui.set_text(k, Some(format!("[Say] Somebody: line {line}")));
            }
        }
    }
}

/// A CPU model of the UI textures: applies `GpuOp`s.
#[derive(Default, PartialEq)]
struct Textures {
    tex: HashMap<TexKey, (Rect, Vec<u32>)>,
}

impl Textures {
    fn at(b: &Rect, x: i32, y: i32) -> usize {
        ((y - b.y0) * b.width() as i32 + (x - b.x0)) as usize
    }

    fn apply(&mut self, ops: &[GpuOp]) {
        for op in ops {
            match op {
                GpuOp::Create { tex, tile_box } => {
                    self.tex
                        .insert(*tex, (*tile_box, vec![0; tile_box.area() as usize]));
                }
                GpuOp::Copy { from, to } => {
                    let Some((fb, fp)) = self.tex.get(from).cloned() else {
                        continue;
                    };
                    if let Some((tb, tp)) = self.tex.get_mut(to) {
                        let o = fb.intersect(tb);
                        for y in o.y0..o.y1 {
                            for x in o.x0..o.x1 {
                                tp[Self::at(tb, x, y)] = fp[Self::at(&fb, x, y)];
                            }
                        }
                    }
                }
                GpuOp::Upload { tex, rect, bytes } => {
                    if let Some((tb, tp)) = self.tex.get_mut(tex) {
                        let w = rect.width() as usize;
                        for (i, c) in bytes.as_chunks::<4>().0.iter().enumerate() {
                            let (x, y) = (rect.x0 + (i % w) as i32, rect.y0 + (i / w) as i32);
                            if tb.contains(x, y) {
                                tp[Self::at(tb, x, y)] = u32::from_le_bytes(*c);
                            }
                        }
                    }
                }
                GpuOp::Clear { tex, rect } => {
                    if let Some((tb, tp)) = self.tex.get_mut(tex) {
                        let o = rect.intersect(tb);
                        for y in o.y0..o.y1 {
                            for x in o.x0..o.x1 {
                                tp[Self::at(tb, x, y)] = 0;
                            }
                        }
                    }
                }
                GpuOp::Promote { surface, layer } => {
                    let back = TexKey {
                        surface: *surface,
                        layer: *layer,
                        slot: Slot::Back,
                    };
                    if let Some(t) = self.tex.remove(&back) {
                        self.tex.insert(
                            TexKey {
                                slot: Slot::Front,
                                ..back
                            },
                            t,
                        );
                    }
                }
                GpuOp::Destroy { tex } => {
                    self.tex.remove(tex);
                }
            }
        }
    }

    /// The front textures (what the composition draws), in key order.
    fn front(&self) -> Vec<(TexKey, Rect, &[u32])> {
        let mut v: Vec<_> = self
            .tex
            .iter()
            .filter(|(k, _)| k.slot == Slot::Front)
            .map(|(k, (b, p))| (*k, *b, p.as_slice()))
            .collect();
        v.sort_by_key(|(k, _, _)| *k);
        v
    }
}

fn stats(v: &mut [Duration]) -> (f64, f64, f64, f64) {
    v.sort_unstable();
    let ms = |d: Duration| d.as_secs_f64() * 1e3;
    let mean = v.iter().map(|d| ms(*d)).sum::<f64>() / v.len().max(1) as f64;
    let p = |q: f64| ms(v[((v.len() as f64 - 1.0) * q).round() as usize]);
    (mean, p(0.5), p(0.99), ms(v[v.len() - 1]))
}

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Idle,
    Hud,
    HudAlone,
    Floaters,
    Rest,
}

impl Phase {
    fn name(self) -> &'static str {
        match self {
            Phase::Idle => "idle",
            Phase::Hud => "hud",
            Phase::HudAlone => "hud-alone",
            Phase::Floaters => "floaters",
            Phase::Rest => "rest",
        }
    }

    /// Whether the phase changes the UI in every frame.
    fn changes(self) -> bool {
        matches!(self, Phase::Hud | Phase::HudAlone | Phase::Floaters)
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let check = args.iter().any(|a| a == "--check");
    let assert = args.iter().any(|a| a == "--assert");
    let frames = args
        .iter()
        .position(|a| a == "--frames")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(600u32);
    let t0 = Instant::now();
    let mut s = scene(false);
    let mut reference = check.then(|| scene(true));
    println!(
        "scene: {} elements, built in {:.1} ms",
        s.ui.element_count(),
        t0.elapsed().as_secs_f64() * 1e3
    );
    let (mut tex, mut ref_tex) = (Textures::default(), Textures::default());
    let mut now = Duration::ZERO;
    let mut failures = 0u32;
    // Start-up: first paints of every surface until quiet.
    for _ in 0..600 {
        now += Duration::from_micros(16_667);
        let f = s.ui.paint(now, &mut Unlimited);
        tex.apply(&f.ops);
        if let Some(r) = reference.as_mut() {
            let g = r.ui.paint(now, &mut Unlimited);
            ref_tex.apply(&g.ops);
        }
        if s.ui.paint_settled() && reference.as_ref().is_none_or(|r| r.ui.paint_settled()) {
            break;
        }
    }
    let mut frame_no = 0u32;
    for phase in [
        Phase::Idle,
        Phase::Hud,
        Phase::HudAlone,
        Phase::Floaters,
        Phase::Rest,
    ] {
        if phase == Phase::HudAlone {
            for k in s.panels.clone() {
                s.ui.control_set_visible(k, false);
            }
            if let Some(r) = reference.as_mut() {
                for k in r.panels.clone() {
                    r.ui.control_set_visible(k, false);
                }
            }
        }
        let before = s.ui.surface_passes();
        let mut times = Vec::with_capacity(frames as usize);
        let (mut bands, mut jobs, mut upload) = (0u64, 0u64, 0u64);
        for _ in 0..frames {
            if phase == Phase::Floaters {
                float_step(&mut s, frame_no, false);
                if let Some(r) = reference.as_mut() {
                    float_step(r, frame_no, false);
                }
                frame_no += 1;
            } else if phase.changes() {
                hud_step(&mut s, frame_no);
                if let Some(r) = reference.as_mut() {
                    hud_step(r, frame_no);
                }
                frame_no += 1;
            }
            now += Duration::from_micros(16_667);
            let mut slice = TimeSlice::new(SLICE);
            let mut unlimited = Unlimited;
            let c = common::thread_cpu(t0);
            let f = if check {
                s.ui.paint(now, &mut unlimited)
            } else {
                s.ui.paint(now, &mut slice)
            };
            times.push(common::thread_cpu(t0).saturating_sub(c));
            bands += u64::from(f.stats.bands);
            jobs += u64::from(f.stats.jobs_started);
            upload += f.stats.upload_bytes;
            tex.apply(&f.ops);
            if let Some(r) = reference.as_mut() {
                let g = r.ui.paint(now, &mut Unlimited);
                ref_tex.apply(&g.ops);
                if g.composition != f.composition || ref_tex.front() != tex.front() {
                    eprintln!(
                        "{}: frame {frame_no} differs from the reference",
                        phase.name()
                    );
                    failures += 1;
                }
            }
        }
        let passes = s.ui.surface_passes();
        let (full, light) = (passes.full - before.full, passes.light - before.light);
        let (mean, p50, p99, max) = stats(&mut times);
        println!(
            "{:<10} frames {frames:>4}  mean {mean:7.3} ms  p50 {p50:7.3}  p99 {p99:7.3}  max {max:7.3}  \
             full {full} light {light}  jobs {jobs} bands {bands} upload {:.1} KiB/frame",
            phase.name(),
            upload as f64 / 1024.0 / f64::from(frames.max(1)),
        );
        if assert {
            let idle_bad = phase == Phase::Idle && (full + light + jobs + bands + upload) > 0;
            let hud_bad = phase.changes() && full * 20 > u64::from(frames);
            let float_bad = phase == Phase::Floaters && (jobs + bands + upload) > 0;
            // The settle repaints keep within the slice (`ui-scaling.md` §6.1 item 4, §8 S7):
            // previews due together settle over several frames.
            let rest_bad = phase == Phase::Rest
                && (s.ui.geometry_preview_pending()
                    || !s.ui.paint_settled()
                    || (!check && max > SLICE.as_secs_f64() * 1e3));
            if idle_bad || hud_bad || float_bad || rest_bad {
                eprintln!("{}: steady-state regression", phase.name());
                failures += 1;
            }
        }
    }
    if !check {
        r13_caps(t0, frames.min(120));
    }
    if failures > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// `render.md` §6.12 R13 at the caps a plugin can reach: a screen-sized transparent layer with
/// 512 nested canvas-resize labels (`MAX_NESTED`), then 3,000 controls drawn after them in a
/// ring around the screen's edge, so that the bounding box of the operations drawn after each
/// entry covers the whole layer while none of them meets a label's path: every region test
/// has to look at every box. All 512 labels move by one whole pixel every frame (no geometry
/// preview at *Se* = 1), so each frame makes 1,024 region tests; the per-frame bound of the
/// test (`R13_CHECKS`) keeps its share of the frame small, and the regions past it are
/// damaged as without R13. Printed only (indicative).
fn r13_caps(t0: Instant, frames: u32) {
    let mut s = scene(false);
    for k in s.panels.clone() {
        s.ui.control_set_visible(k, false);
    }
    let layer = window(&mut s.ui, [0, 0, SCREEN.0, SCREEN.1]);
    let labels: Vec<ElementKey> = (0..512)
        .map(|i| {
            let k = label(
                &mut s.ui,
                layer,
                [200 + (i % 32) * 48, 200 + (i / 32) * 40, 30, 16],
                "-7",
            );
            s.ui.control_set_stretch_mode(k, 3);
            k
        })
        .collect();
    for i in 0..3_000 {
        // 750 squares on each edge, 2 × 2 units, inside the outer 40 units.
        let (x, y) = match i % 4 {
            0 => ((i / 4) * 2 % SCREEN.0, 10),
            1 => ((i / 4) * 2 % SCREEN.0, SCREEN.1 - 30),
            2 => (10, (i / 4) * 2 % SCREEN.1),
            _ => (SCREEN.0 - 30, (i / 4) * 2 % SCREEN.1),
        };
        let c = control(&mut s.ui, ControlKind::Control, layer, [x, y, 2, 2]);
        s.ui.control_set_back_colour(c, Some([0.9, 0.9, 0.9, 1.0]));
    }
    let mut now = Duration::from_secs(1_000);
    for _ in 0..600 {
        now += Duration::from_micros(16_667);
        let _ = s.ui.paint(now, &mut Unlimited);
        if s.ui.paint_settled() {
            break;
        }
    }
    let mut times = Vec::with_capacity(frames as usize);
    let (mut jobs, mut bands) = (0u64, 0u64);
    for f in 0..frames {
        let dx = if f % 2 == 0 { 1 } else { -1 };
        for &k in &labels {
            let (x, y) = s.ui.control_position(k);
            s.ui.control_set_position(k, x + dx, y);
        }
        now += Duration::from_micros(16_667);
        let mut slice = TimeSlice::new(SLICE);
        let c = common::thread_cpu(t0);
        let fr = s.ui.paint(now, &mut slice);
        times.push(common::thread_cpu(t0).saturating_sub(c));
        jobs += u64::from(fr.stats.jobs_started);
        bands += u64::from(fr.stats.bands);
    }
    let (mean, p50, p99, max) = stats(&mut times);
    println!(
        "{:<10} frames {frames:>4}  mean {mean:7.3} ms  p50 {p50:7.3}  p99 {p99:7.3}  max {max:7.3}  \
         jobs {jobs} bands {bands}  (512 nested entries move every frame)",
        "r13-caps",
    );
}
