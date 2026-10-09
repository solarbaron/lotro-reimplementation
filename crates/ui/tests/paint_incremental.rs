// SPDX-License-Identifier: GPL-3.0-or-later
//! The incremental paint step gives the frames of the full one (decision 0015 Decision
//! 1.4–1.6). `Ui::paint` runs the full surface pass (surfaces, coverage regions, groups,
//! composition nodes, a damage walk of every surface) only when something structural
//! changed, and otherwise walks only the subtrees of the elements that changed; image
//! arrivals damage only the elements that waited for an image; glyph runs and op areas are
//! cached. Two identical UIs get the same random edits, one with the reference paint step
//! (the full pass on every frame, no glyph-run cache, everything repainted when images
//! arrive): every frame's composition list and the GPU textures that its operations
//! leave must be equal. Synthetic controls, images and text (Tessera's bundled font).

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use proptest::prelude::*;
use tessera_formats::Did;
use tessera_formats::ui_layout::UiLayout;
use tessera_material::ui_image::{PixelLayout, UiImage};
use tessera_ui::paint::{GpuOp, Rect, Slot, TexKey, Unlimited};
use tessera_ui::text::{CharListBuilder, DEFAULT_RANGES, LayoutParams, TesseraFont, layout};
use tessera_ui::{
    Background, ControlKind, ElementKey, ImageRef, ImageState, MemoryEnv, PreparedText,
    TextRequest, Ui, UiEnv,
};

const TEXTS: [&str; 4] = [
    "alpha",
    "Beta gamma",
    "12.5S, 3.1W",
    "a longer line of text",
];
const IMAGES: [u32; 3] = [0x4100_0001, 0x4100_0002, 0x4100_0003];

fn image(w: u32, h: u32, seed: u32) -> Arc<UiImage> {
    let mut x = seed.wrapping_mul(2_654_435_761).max(1);
    let px = (0..w * h)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            // Opaque, translucent and transparent texels.
            match x % 3 {
                0 => x | 0xFF00_0000,
                1 => (x & 0x00FF_FFFF) | 0x8000_0000,
                _ => 0,
            }
        })
        .collect();
    Arc::new(UiImage::new(w, h, w, h, PixelLayout::A8R8G8B8, px).expect("valid image"))
}

fn env() -> MemoryEnv {
    let mut env = MemoryEnv::default();
    for (i, d) in IMAGES.iter().enumerate() {
        let side = 8 + 8 * i as u32;
        env.images
            .insert(ImageRef::Did(*d), image(side, side + 3, i as u32 + 1));
    }
    let font = TesseraFont::bundled().expect("bundled font");
    let face = Arc::new(font.rasterise(12, DEFAULT_RANGES).expect("rasterise"));
    let fonts = vec![Some(face)];
    for t in TEXTS {
        let mut b = CharListBuilder::new();
        b.colour(0xFFE0_D0B0).push_str(t);
        let chars = b.finish();
        let p = LayoutParams {
            width: 90,
            height: 40,
            ..LayoutParams::default()
        };
        let l = layout(&chars, &fonts, &p, None).expect("layout");
        env.texts.insert(
            t.to_string(),
            Arc::new(PreparedText {
                layout: l,
                chars,
                fonts: fonts.clone(),
                outline: true,
                outline_colour: 0xFF00_0000,
                truncated: false,
                extent: (90, 40),
            }),
        );
    }
    env
}

/// [`MemoryEnv`] in which the last image is still loading until `arrived` is set.
struct LoadingEnv {
    inner: MemoryEnv,
    arrived: Arc<AtomicBool>,
}

impl UiEnv for LoadingEnv {
    fn layout(&mut self, did: Did) -> Option<Arc<UiLayout>> {
        self.inner.layout(did)
    }

    fn image(&mut self, image: &ImageRef) -> ImageState {
        if *image == ImageRef::Did(IMAGES[2]) && !self.arrived.load(Ordering::Relaxed) {
            return ImageState::Pending;
        }
        self.inner.image(image)
    }

    fn shape_text(&mut self, request: &TextRequest<'_>) -> Option<Arc<PreparedText>> {
        self.inner.shape_text(request)
    }
}

/// The scene: three windows; in each a container with eight children (images, labels);
/// a stretch-mode control (its own surface) in the first and a child window in the second.
struct Scene {
    ui: Ui,
    arrived: Arc<AtomicBool>,
    /// Repaint everything when images arrive (the reference for the waiter list).
    full: bool,
    /// Controls the edits pick from (index stable across both UIs).
    controls: Vec<ElementKey>,
    containers: Vec<ElementKey>,
    nested: Vec<ElementKey>,
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

fn scene(full: bool) -> Scene {
    let arrived = Arc::new(AtomicBool::new(false));
    let env = LoadingEnv {
        inner: env(),
        arrived: Arc::clone(&arrived),
    };
    let mut ui = Ui::new(Box::new(env), tessera_ui::Vocab::default(), (640, 480));
    ui.set_reference_paint(full);
    let mut s = Scene {
        ui,
        arrived,
        full,
        controls: Vec::new(),
        containers: Vec::new(),
        nested: Vec::new(),
        windows: Vec::new(),
    };
    for w in 0..3 {
        let win = s.ui.create_control(ControlKind::Window, 1).expect("window");
        s.ui.control_set_position(win, 20 + w * 200, 30 + w * 40);
        s.ui.control_set_size(win, 180, 260);
        s.ui.control_set_visible(win, true);
        s.ui.control_set_background(win, Background::Did(IMAGES[0]));
        s.windows.push(win);
        let body = control(&mut s.ui, ControlKind::Control, win, [5, 20, 170, 230]);
        s.containers.push(body);
        for i in 0..8 {
            let kind = if i % 2 == 0 {
                ControlKind::Control
            } else {
                ControlKind::Label
            };
            let c = control(&mut s.ui, kind, body, [(i % 3) * 50, (i / 3) * 60, 60, 45]);
            if kind == ControlKind::Label {
                s.ui.set_text(c, Some(TEXTS[i as usize % TEXTS.len()].to_string()));
            } else {
                s.ui.control_set_background(c, Background::Did(IMAGES[i as usize % 3]));
            }
            s.controls.push(c);
        }
    }
    // A control with its own surface (stretch mode 1).
    let st = control(
        &mut s.ui,
        ControlKind::Control,
        s.containers[0],
        [30, 100, 40, 40],
    );
    s.ui.control_set_stretch_mode(st, 1);
    s.ui.control_set_background(st, Background::Did(IMAGES[1]));
    s.nested.push(st);
    // A child window (a nested surface) that turns.
    let cw = s.ui.create_control(ControlKind::Window, 1).expect("window");
    s.ui.control_set_parent(cw, Some(s.windows[1]))
        .expect("parent");
    s.ui.control_set_position(cw, 60, 60);
    s.ui.control_set_size(cw, 30, 30);
    s.ui.control_set_visible(cw, true);
    s.ui.control_set_background(cw, Background::Did(IMAGES[2]));
    s.nested.push(cw);
    s
}

#[derive(Debug, Clone)]
enum Edit {
    Move(usize, i32, i32),
    Resize(usize, i32, i32),
    Visible(usize, bool),
    Text(usize, usize),
    Background(usize, usize),
    BackColour(usize, u8),
    Reparent(usize, usize),
    ZOrder(usize, i32),
    Destroy(usize),
    Rotate(i16),
    MoveNested(usize, i32, i32),
    StretchSize(i32),
    WindowMove(usize, i32, i32),
    WindowOpacity(usize, u8),
    ContainerMove(usize, i32, i32),
    ContainerVisible(usize, bool),
    NestedVisible(usize, bool),
    ImagesArrived,
    Nothing,
}

fn edit() -> impl Strategy<Value = Edit> {
    prop_oneof![
        (0..24usize, -6..7i32, -6..7i32).prop_map(|(i, x, y)| Edit::Move(i, x, y)),
        (0..24usize, 0..80i32, 0..70i32).prop_map(|(i, w, h)| Edit::Resize(i, w, h)),
        (0..24usize, any::<bool>()).prop_map(|(i, v)| Edit::Visible(i, v)),
        (0..24usize, 0..TEXTS.len()).prop_map(|(i, t)| Edit::Text(i, t)),
        (0..24usize, 0..4usize).prop_map(|(i, b)| Edit::Background(i, b)),
        (0..24usize, any::<u8>()).prop_map(|(i, c)| Edit::BackColour(i, c)),
        (0..24usize, 0..3usize).prop_map(|(i, p)| Edit::Reparent(i, p)),
        (0..24usize, -2..3i32).prop_map(|(i, z)| Edit::ZOrder(i, z)),
        (0..24usize).prop_map(Edit::Destroy),
        (-2..400i16).prop_map(Edit::Rotate),
        (0..2usize, -9..10i32, -9..10i32).prop_map(|(i, x, y)| Edit::MoveNested(i, x, y)),
        (10..60i32).prop_map(Edit::StretchSize),
        (0..3usize, -9..10i32, -9..10i32).prop_map(|(i, x, y)| Edit::WindowMove(i, x, y)),
        (0..3usize, any::<u8>()).prop_map(|(i, o)| Edit::WindowOpacity(i, o)),
        (0..3usize, -9..10i32, -9..10i32).prop_map(|(i, x, y)| Edit::ContainerMove(i, x, y)),
        (0..3usize, any::<bool>()).prop_map(|(i, v)| Edit::ContainerVisible(i, v)),
        (0..2usize, any::<bool>()).prop_map(|(i, v)| Edit::NestedVisible(i, v)),
        Just(Edit::ImagesArrived),
        Just(Edit::Nothing),
    ]
}

fn apply(s: &mut Scene, e: &Edit) {
    let ui = &mut s.ui;
    let live = |ui: &Ui, k: ElementKey| ui.exists(k);
    match *e {
        Edit::Move(i, dx, dy) => {
            let k = s.controls[i];
            if live(ui, k) {
                let (x, y) = ui.control_position(k);
                ui.control_set_position(k, x + dx, y + dy);
            }
        }
        Edit::Resize(i, w, h) => {
            let k = s.controls[i];
            if live(ui, k) {
                ui.control_set_size(k, w, h);
            }
        }
        Edit::Visible(i, v) => {
            let k = s.controls[i];
            if live(ui, k) {
                ui.control_set_visible(k, v);
            }
        }
        Edit::Text(i, t) => {
            let k = s.controls[i];
            if live(ui, k) {
                ui.set_text(k, Some(TEXTS[t].to_string()));
            }
        }
        Edit::Background(i, b) => {
            let k = s.controls[i];
            if live(ui, k) {
                let bg = IMAGES
                    .get(b)
                    .map_or(Background::None, |d| Background::Did(*d));
                ui.control_set_background(k, bg);
            }
        }
        Edit::BackColour(i, c) => {
            let k = s.controls[i];
            if live(ui, k) {
                let v = f32::from(c) / 255.0;
                ui.control_set_back_colour(k, Some([v, 0.5, 1.0 - v, 0.5 + v / 2.0]));
            }
        }
        Edit::Reparent(i, p) => {
            let k = s.controls[i];
            if live(ui, k) {
                let _ = ui.control_set_parent(k, Some(s.containers[p]));
            }
        }
        Edit::ZOrder(i, z) => {
            let k = s.controls[i];
            if live(ui, k) {
                ui.control_set_z_order(k, z);
            }
        }
        Edit::Destroy(i) => {
            let k = s.controls[i];
            if live(ui, k) {
                ui.destroy_control(k);
            }
        }
        Edit::Rotate(a) => {
            // Angle 0 first, then turns: the first rotation changes the coverage region,
            // later ones only the composition entry (Decision 1.7).
            let _ = ui.window_set_rotation(s.nested[1], [0.0, 0.0, f32::from(a.max(0))]);
        }
        Edit::MoveNested(i, dx, dy) => {
            let k = s.nested[i];
            let (x, y) = ui.control_position(k);
            ui.control_set_position(k, x + dx, y + dy);
        }
        Edit::StretchSize(w) => ui.control_set_size(s.nested[0], w, w),
        Edit::WindowMove(i, dx, dy) => {
            let k = s.windows[i];
            let (x, y) = ui.control_position(k);
            ui.control_set_position(k, x + dx, y + dy);
        }
        Edit::WindowOpacity(i, o) => ui.control_set_opacity(s.windows[i], f32::from(o) / 255.0),
        Edit::ContainerMove(i, dx, dy) => {
            let k = s.containers[i];
            let (x, y) = ui.control_position(k);
            ui.control_set_position(k, x + dx, y + dy);
        }
        Edit::ContainerVisible(i, v) => ui.control_set_visible(s.containers[i], v),
        Edit::NestedVisible(i, v) => ui.control_set_visible(s.nested[i], v),
        Edit::ImagesArrived => {
            // The incremental UI damages the elements that waited; the reference repaints
            // every element.
            s.arrived.store(true, Ordering::Relaxed);
            ui.images_arrived();
            if s.full {
                ui.invalidate_paint();
            }
        }
        Edit::Nothing => {}
    }
}

/// A CPU model of the UI textures that applies `GpuOp`s.
#[derive(Default, PartialEq, Debug)]
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
                    let (tb, tp) = self.tex.get_mut(tex).expect("upload into a texture");
                    let w = rect.width() as usize;
                    for (i, c) in bytes.as_chunks::<4>().0.iter().enumerate() {
                        let (x, y) = (rect.x0 + (i % w) as i32, rect.y0 + (i / w) as i32);
                        assert!(tb.contains(x, y), "upload inside the texture box");
                        tp[Self::at(tb, x, y)] = u32::from_le_bytes(*c);
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

    /// The front textures only (what the composition draws).
    fn front(&self) -> Vec<(TexKey, Rect, Vec<u32>)> {
        let mut v: Vec<_> = self
            .tex
            .iter()
            .filter(|(k, _)| k.slot == Slot::Front)
            .map(|(k, (b, p))| (*k, *b, p.clone()))
            .collect();
        v.sort_by_key(|(k, _, _)| *k);
        v
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    #[test]
    fn incremental_frames_equal_full_frames(
        frames in prop::collection::vec(prop::collection::vec(edit(), 0..4), 1..24)
    ) {
        let mut inc = scene(false);
        let mut full = scene(true);
        let (mut ti, mut tf) = (Textures::default(), Textures::default());
        let mut t = Duration::ZERO;
        let mut step = |inc: &mut Scene, full: &mut Scene, ti: &mut Textures, tf: &mut Textures| {
            t += Duration::from_millis(16);
            let a = inc.ui.paint(t, &mut Unlimited);
            let b = full.ui.paint(t, &mut Unlimited);
            ti.apply(&a.ops);
            tf.apply(&b.ops);
            (a.composition, b.composition)
        };
        // First paints.
        for _ in 0..8 {
            let (a, b) = step(&mut inc, &mut full, &mut ti, &mut tf);
            prop_assert_eq!(a, b);
        }
        prop_assert_eq!(ti.front(), tf.front());
        for (n, edits) in frames.iter().enumerate() {
            for e in edits {
                apply(&mut inc, e);
                apply(&mut full, e);
            }
            let (a, b) = step(&mut inc, &mut full, &mut ti, &mut tf);
            prop_assert_eq!(a, b, "composition of frame {}", n);
            prop_assert!(ti.front() == tf.front(), "textures after frame {}", n);
        }
    }
}

/// A label wider than its clip on both sides moves by one pixel: its clip stays, its text
/// origin changes, so its cached glyph runs must not be reused.
#[test]
fn moved_text_with_the_same_clip_is_drawn_again() {
    let mut uis: Vec<(Ui, ElementKey)> = [false, true]
        .into_iter()
        .map(|reference| {
            let arrived = Arc::new(AtomicBool::new(true));
            let env = LoadingEnv {
                inner: env(),
                arrived,
            };
            let mut ui = Ui::new(Box::new(env), tessera_ui::Vocab::default(), (320, 200));
            ui.set_reference_paint(reference);
            let win = ui.create_control(ControlKind::Window, 1).expect("window");
            ui.control_set_position(win, 10, 10);
            ui.control_set_size(win, 120, 60);
            ui.control_set_visible(win, true);
            let clip = control(&mut ui, ControlKind::Control, win, [20, 10, 50, 30]);
            let label = control(&mut ui, ControlKind::Label, clip, [-40, 0, 200, 30]);
            ui.set_text(label, Some(TEXTS[3].to_string()));
            (ui, label)
        })
        .collect();
    let mut tex = [Textures::default(), Textures::default()];
    let mut t = Duration::ZERO;
    for step in 0..12 {
        t += Duration::from_millis(16);
        let mut comps = Vec::new();
        for ((ui, label), tx) in uis.iter_mut().zip(tex.iter_mut()) {
            if step >= 4 {
                let (x, y) = ui.control_position(*label);
                ui.control_set_position(*label, x - 1, y);
            }
            let f = ui.paint(t, &mut Unlimited);
            tx.apply(&f.ops);
            comps.push(f.composition);
        }
        assert_eq!(comps[0], comps[1], "composition of frame {step}");
        assert!(
            tex[0].front() == tex[1].front(),
            "textures after frame {step}"
        );
    }
}
