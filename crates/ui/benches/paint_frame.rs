// SPDX-License-Identifier: GPL-3.0-or-later
//! Frame-loop step 8 benchmark (`Ui::paint`) on a synthetic UI shaped like the scripted run
//! of decision 0015 §6: about 5,000 plugin controls in a dozen windows at 3840 × 2160, a
//! minimap-like window whose map control moves every frame, a rotated child window, a
//! coordinate label, a screen-sized window with a full-size background, and text from
//! Tessera's bundled fallback font. Synthetic images only.
//!
//! Run with `cargo bench -p tessera-ui --bench paint_frame` (a release build). Options:
//!
//! - `--check`: paints with an unlimited budget and prints a hash of the composition list
//!   of every frame and of the final GPU texture contents (the `GpuOp`s applied to a CPU
//!   model of the textures) per phase. Two builds give the same output exactly when these
//!   hashes are equal.
//! - `--frames N`: frames per phase (default 600).
//!
//! Times are the paint call's **thread CPU time** (`CLOCK_THREAD_CPUTIME_ID`), so waiting
//! for a core on a loaded machine is not counted; the 4 ms paint slice itself still uses
//! wall-clock time, as in the client. Results are **indicative** unless taken on the reference machine under rule 1 of
//! `docs/perf/reference-machine.md`; record the conditions next to every number.

// The one `unsafe` call reads the thread's CPU clock (`thread_cpu`); std has no safe API.
#![allow(unsafe_code)]

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_formats::Did;
use tessera_formats::ui_layout::UiLayout;
use tessera_material::ui_image::{PixelLayout, UiImage};
use tessera_ui::paint::{
    CompositionList, GpuOp, PaintBudget, Rect, Slot, TexKey, TimeSlice, Unlimited,
};
use tessera_ui::text::{
    CharListBuilder, DEFAULT_RANGES, FontFace, LayoutParams, TesseraFont, layout,
};
use tessera_ui::{
    Background, ControlKind, ElementKey, ImageRef, ImageState, PreparedText, TextRequest, Ui,
    UiEnv, Vocab,
};

const SCREEN: (i32, i32) = (3840, 2160);
const IMG_PANEL: u32 = 0x4100_0001;
const IMG_ICON: u32 = 0x4100_0002;
const IMG_MAP: u32 = 0x4100_0003;
const IMG_SCREEN: u32 = 0x4100_0004;
const IMG_FRAME: u32 = 0x4100_0005;

/// An environment with synthetic images and the bundled fallback font.
struct BenchEnv {
    images: HashMap<ImageRef, Arc<UiImage>>,
    face: Arc<FontFace>,
    shaped: u64,
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
        self.shaped += 1;
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

fn noise(w: u32, h: u32, seed: u32, alpha: Option<u32>) -> Arc<UiImage> {
    let mut x = seed.wrapping_mul(2_654_435_761).max(1);
    let px = (0..w * h)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            match alpha {
                Some(a) => (a << 24) | (x & 0xFF_FFFF),
                None => x,
            }
        })
        .collect();
    Arc::new(UiImage::new(w, h, w, h, PixelLayout::A8R8G8B8, px).expect("valid image"))
}

/// A frame with a transparent middle (panel art).
fn frame_image(side: u32) -> Arc<UiImage> {
    let px = (0..side * side)
        .map(|i| {
            let (x, y) = (i % side, i / side);
            let edge = x < 6 || y < 6 || x >= side - 6 || y >= side - 6;
            if edge { 0xFF40_3020 } else { 0x2010_0800 }
        })
        .collect();
    Arc::new(UiImage::new(side, side, side, side, PixelLayout::A8R8G8B8, px).expect("valid"))
}

struct Scene {
    ui: Ui,
    map: ElementKey,
    coords: ElementKey,
    marker: ElementKey,
    chat: Vec<ElementKey>,
    windows: Vec<ElementKey>,
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

fn scene() -> Scene {
    let font = TesseraFont::bundled().expect("bundled font");
    let face = Arc::new(font.rasterise(14, DEFAULT_RANGES).expect("rasterise"));
    let mut images = HashMap::new();
    images.insert(ImageRef::Did(IMG_PANEL), noise(64, 64, 1, Some(0xC0)));
    images.insert(ImageRef::Did(IMG_ICON), noise(32, 32, 2, None));
    images.insert(ImageRef::Did(IMG_MAP), noise(512, 512, 3, Some(0xFF)));
    images.insert(ImageRef::Did(IMG_SCREEN), noise(256, 256, 4, Some(0x30)));
    images.insert(ImageRef::Did(IMG_FRAME), frame_image(128));
    let env = BenchEnv {
        images,
        face,
        shaped: 0,
    };
    let mut ui = Ui::new(Box::new(env), Vocab::default(), SCREEN);
    // The screen-sized window with a full-size background (drawn first, below the rest).
    let screen = window(&mut ui, [0, 0, SCREEN.0, SCREEN.1]);
    ui.control_set_background(screen, Background::Did(IMG_SCREEN));
    let mut windows = vec![screen];
    // Twelve panels of 12 × 16 slots: icon, frame and a label each.
    for i in 0..12 {
        let (col, row) = (i % 6, i / 6);
        let w = window(&mut ui, [40 + col * 620, 300 + row * 820, 600, 780]);
        ui.control_set_background(w, Background::Did(IMG_PANEL));
        let body = control(&mut ui, ControlKind::Control, w, [10, 30, 580, 740]);
        let title = control(&mut ui, ControlKind::Label, w, [10, 6, 300, 20]);
        ui.set_text(title, Some(format!("Panel {i}")));
        for s in 0..(12 * 16) {
            let (x, y) = ((s % 12) * 48, (s / 12) * 46);
            let slot = control(&mut ui, ControlKind::Control, body, [x, y, 44, 44]);
            ui.control_set_background(slot, Background::Did(IMG_ICON));
            let fr = control(&mut ui, ControlKind::Control, slot, [0, 0, 44, 44]);
            ui.control_set_background(fr, Background::Did(IMG_FRAME));
            let lab = control(&mut ui, ControlKind::Label, slot, [2, 28, 40, 16]);
            ui.set_text(lab, Some(format!("{}", s % 97)));
        }
        windows.push(w);
    }
    // A chat-like window: 12 lines of text.
    let chat_w = window(&mut ui, [40, 40, 600, 240]);
    ui.control_set_background(chat_w, Background::Did(IMG_PANEL));
    let chat = (0..12)
        .map(|l| {
            let k = control(
                &mut ui,
                ControlKind::Label,
                chat_w,
                [8, 8 + l * 18, 584, 18],
            );
            ui.set_text(k, Some(format!("[Channel] Somebody: line {l} of the log")));
            k
        })
        .collect();
    windows.push(chat_w);
    // The minimap: a 170 × 170 viewport, a 512 × 512 map moved every frame, a frame drawn
    // over it, a coordinate label and a rotated child window (the marker).
    let mm = window(&mut ui, [3600, 12, 224, 280]);
    ui.control_set_background(mm, Background::Did(IMG_PANEL));
    let view = control(&mut ui, ControlKind::Control, mm, [27, 27, 170, 170]);
    let map = control(&mut ui, ControlKind::Control, view, [-171, -171, 512, 512]);
    ui.control_set_background(map, Background::Did(IMG_MAP));
    let rim = control(&mut ui, ControlKind::Control, mm, [20, 20, 184, 184]);
    ui.control_set_background(rim, Background::Did(IMG_FRAME));
    let coords = control(&mut ui, ControlKind::Label, mm, [20, 220, 184, 20]);
    ui.set_text(coords, Some("0.0S, 0.0W".into()));
    let marker = ui.create_control(ControlKind::Window, 1).expect("marker");
    ui.control_set_parent(marker, Some(mm)).expect("parent");
    ui.control_set_position(marker, 98, 98);
    ui.control_set_size(marker, 28, 28);
    ui.control_set_visible(marker, true);
    ui.control_set_background(marker, Background::Did(IMG_ICON));
    windows.push(mm);
    Scene {
        ui,
        map,
        coords,
        marker,
        chat,
        windows,
    }
}

/// A CPU model of the UI textures: applies `GpuOp`s.
#[derive(Default)]
struct Textures {
    tex: HashMap<TexKey, (Rect, Vec<u32>)>,
}

impl Textures {
    fn apply(&mut self, ops: &[GpuOp]) {
        for op in ops {
            match op {
                GpuOp::Create { tex, tile_box } => {
                    let n = tile_box.area() as usize;
                    self.tex.insert(*tex, (*tile_box, vec![0; n]));
                }
                GpuOp::Copy { from, to } => {
                    let Some((fb, fp)) = self.tex.get(from).cloned() else {
                        continue;
                    };
                    if let Some((tb, tp)) = self.tex.get_mut(to) {
                        let o = fb.intersect(tb);
                        for y in o.y0..o.y1 {
                            for x in o.x0..o.x1 {
                                let s = ((y - fb.y0) * fb.width() as i32 + (x - fb.x0)) as usize;
                                let d = ((y - tb.y0) * tb.width() as i32 + (x - tb.x0)) as usize;
                                tp[d] = fp[s];
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
                                let d = ((y - tb.y0) * tb.width() as i32 + (x - tb.x0)) as usize;
                                tp[d] = u32::from_le_bytes(*c);
                            }
                        }
                    }
                }
                GpuOp::Clear { tex, rect } => {
                    if let Some((tb, tp)) = self.tex.get_mut(tex) {
                        let o = rect.intersect(tb);
                        for y in o.y0..o.y1 {
                            for x in o.x0..o.x1 {
                                let d = ((y - tb.y0) * tb.width() as i32 + (x - tb.x0)) as usize;
                                tp[d] = 0;
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

    /// Hash of the front textures (box and texels), in key order.
    fn hash(&self) -> u64 {
        let mut keys: Vec<&TexKey> = self.tex.keys().filter(|k| k.slot == Slot::Front).collect();
        keys.sort();
        let mut h = DefaultHasher::new();
        for k in keys {
            k.hash(&mut h);
            if let Some((b, p)) = self.tex.get(k) {
                b.hash(&mut h);
                p.hash(&mut h);
            }
        }
        h.finish()
    }
}

fn hash_composition(c: &CompositionList, h: &mut DefaultHasher) {
    format!("{c:?}").hash(h);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Walk,
    WalkImages,
    Chat,
    Resize,
}

impl Phase {
    fn name(self) -> &'static str {
        match self {
            Phase::Idle => "idle",
            Phase::Walk => "walk",
            Phase::WalkImages => "walk+images",
            Phase::Chat => "chat",
            Phase::Resize => "resize",
        }
    }
}

fn step(s: &mut Scene, phase: Phase, f: u32) {
    match phase {
        Phase::Idle => {}
        Phase::Walk | Phase::WalkImages => {
            let (x, y) = s.ui.control_position(s.map);
            let dx = if (f / 150).is_multiple_of(2) { 1 } else { -1 };
            s.ui.control_set_position(s.map, x + dx, y + i32::from(f.is_multiple_of(3)));
            if f.is_multiple_of(4) {
                s.ui.set_text(s.coords, Some(format!("{}.{}S, 12.3W", f / 40, f % 10)));
            }
            s.ui.window_set_rotation(s.marker, [0.0, 0.0, (f % 360) as f32]);
            if phase == Phase::WalkImages && f.is_multiple_of(10) {
                s.ui.images_arrived();
            }
        }
        Phase::Resize => {
            // `SetSize` on a panel every frame (a layout change of its group).
            let w = s.windows[1];
            let d = i32::try_from(f % 40).unwrap_or(0);
            s.ui.control_set_size(w, 560 + d, 740 + d);
        }
        Phase::Chat => {
            if f.is_multiple_of(6) {
                let n = s.chat.len();
                for (i, k) in s.chat.clone().into_iter().enumerate() {
                    let line = (f / 6) as usize + i;
                    let _ = n;
                    s.ui.set_text(
                        k,
                        Some(format!("[Channel] Somebody: line {line} of the log")),
                    );
                }
            }
        }
    }
}

/// CPU time of the calling thread so far (`CLOCK_THREAD_CPUTIME_ID`), so that waiting for
/// a core on a loaded machine is not counted. Falls back to wall-clock time since `start`.
fn thread_cpu(start: Instant) -> Duration {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `ts` is a valid, writable `timespec` owned by this frame, and the clock id is
    // a constant the C library defines; `clock_gettime` writes only into `ts`.
    let rc = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &raw mut ts) };
    if rc != 0 {
        return start.elapsed();
    }
    Duration::new(
        u64::try_from(ts.tv_sec).unwrap_or(0),
        u32::try_from(ts.tv_nsec).unwrap_or(0),
    )
}

fn stats(v: &mut [Duration]) -> (f64, f64, f64, f64) {
    v.sort_unstable();
    let ms = |d: Duration| d.as_secs_f64() * 1e3;
    let mean = v.iter().map(|d| ms(*d)).sum::<f64>() / v.len().max(1) as f64;
    let p = |q: f64| ms(v[((v.len() as f64 - 1.0) * q).round() as usize]);
    (mean, p(0.5), p(0.99), ms(v[v.len() - 1]))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let check = args.iter().any(|a| a == "--check");
    let frames = args
        .iter()
        .position(|a| a == "--frames")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(600u32);
    let t0 = Instant::now();
    let mut s = scene();
    println!(
        "scene: {} elements, {} windows, built in {:.1} ms",
        s.ui.element_count(),
        s.windows.len(),
        t0.elapsed().as_secs_f64() * 1e3
    );
    let mut tex = Textures::default();
    let mut now = Duration::ZERO;
    let paint = |s: &mut Scene, tex: &mut Textures, now: &mut Duration, h: &mut DefaultHasher| {
        *now += Duration::from_micros(16_667);
        let mut slice = TimeSlice::new(Duration::from_millis(4));
        let mut unlimited = Unlimited;
        let budget: &mut dyn PaintBudget = if check { &mut unlimited } else { &mut slice };
        let c = thread_cpu(t0);
        let frame = s.ui.paint(*now, budget);
        let d = thread_cpu(t0).saturating_sub(c);
        if check {
            tex.apply(&frame.ops);
            hash_composition(&frame.composition, h);
        }
        (d, frame.stats.bands)
    };
    // Start-up: first paints of every surface until quiet.
    let mut h = DefaultHasher::new();
    let mut startup = Vec::new();
    for _ in 0..400 {
        let (d, bands) = paint(&mut s, &mut tex, &mut now, &mut h);
        startup.push(d);
        if bands == 0 && startup.len() > 60 {
            break;
        }
    }
    let (mean, p50, p99, max) = stats(&mut startup);
    println!(
        "{:<12} frames {:>4}  mean {mean:7.3} ms  p50 {p50:7.3}  p99 {p99:7.3}  max {max:7.3}{}",
        "startup",
        startup.len(),
        if check {
            format!("  comp {:016x}  tex {:016x}", h.finish(), tex.hash())
        } else {
            String::new()
        }
    );
    for phase in [
        Phase::Idle,
        Phase::Walk,
        Phase::WalkImages,
        Phase::Chat,
        Phase::Resize,
    ] {
        let mut h = DefaultHasher::new();
        let mut v = Vec::with_capacity(frames as usize);
        for f in 0..frames {
            step(&mut s, phase, f);
            let (d, _) = paint(&mut s, &mut tex, &mut now, &mut h);
            v.push(d);
        }
        // Quiet frames so that every job ends before the hash.
        for _ in 0..30 {
            let _ = paint(&mut s, &mut tex, &mut now, &mut h);
        }
        let (mean, p50, p99, max) = stats(&mut v);
        println!(
            "{:<12} frames {:>4}  mean {mean:7.3} ms  p50 {p50:7.3}  p99 {p99:7.3}  max {max:7.3}{}",
            phase.name(),
            frames,
            if check {
                format!("  comp {:016x}  tex {:016x}", h.finish(), tex.hash())
            } else {
                String::new()
            }
        );
    }
}
