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
//!
//! The scene also has a transparent layer window with nested surface owners that move,
//! grow, change their node scale, text, opacity and parent (like floating combat text),
//! and the edits move subtrees that hold surfaces among their siblings and between
//! windows: the light pass takes the changes of a surface owner itself
//! (`Ui::light_owner`) and must still give the frames of the full pass. A second property
//! runs the same edits under a UI screen, where node scales change the canvas map, at
//! effective scales 1, 1.25 and 1.5 (every one a map that is not the identity: the UI
//! screen has at least 1024 × 768 units).

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use proptest::prelude::*;
use tessera_formats::Did;
use tessera_formats::ui_layout::UiLayout;
use tessera_material::ui_image::{PixelLayout, UiImage};
use tessera_ui::paint::{BandBudget, GpuOp, Rect, Slot, TexKey, Unlimited};
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

/// A UI screen: the window's physical size and the effective scale *Se*.
type Screen = Option<(i32, i32, f64)>;

/// The UI screens of the properties: *K* = 0.625 at *Se* = 1 (the 1024 × 768 floor over a
/// 640 × 480 window), and *Se* = 1.25 and 1.5 over the same 1024 × 768 units.
const SCREENS: [(i32, i32, f64); 3] = [(640, 480, 1.0), (1280, 960, 1.25), (1536, 1152, 1.5)];

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
    /// The transparent layer window (a group root) and the owners nested in it.
    layer: ElementKey,
    floaters: Vec<ElementKey>,
}

fn control(ui: &mut Ui, kind: ControlKind, parent: ElementKey, b: [i32; 4]) -> ElementKey {
    let k = ui.create_control(kind, 1).expect("control");
    ui.control_set_parent(k, Some(parent)).expect("parent");
    ui.control_set_position(k, b[0], b[1]);
    ui.control_set_size(k, b[2], b[3]);
    ui.control_set_visible(k, true);
    k
}

fn scene_with(full: bool, screen: Screen) -> Scene {
    let arrived = Arc::new(AtomicBool::new(false));
    let env = LoadingEnv {
        inner: env(),
        arrived: Arc::clone(&arrived),
    };
    let mut ui = Ui::new(Box::new(env), tessera_ui::Vocab::default(), (640, 480));
    if let Some((wp, hp, se)) = screen {
        let f = tessera_ui::scaling::UiScreen::new(wp, hp, se).expect("screen");
        ui.set_ui_screen(f, tessera_ui::scaling::safe_area(&f, None));
    }
    ui.set_reference_paint(full);
    let root = ui.screen_root();
    let mut s = Scene {
        ui,
        arrived,
        full,
        controls: Vec::new(),
        containers: Vec::new(),
        nested: Vec::new(),
        windows: Vec::new(),
        layer: root,
        floaters: Vec::new(),
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
    // The layer: a transparent window over the others with three nested owners (canvas
    // resize mode), two labels and a picture.
    let layer = s.ui.create_control(ControlKind::Window, 1).expect("window");
    s.ui.control_set_position(layer, 0, 0);
    s.ui.control_set_size(layer, 640, 480);
    s.ui.control_set_visible(layer, true);
    s.layer = layer;
    for i in 0..3 {
        let kind = if i < 2 {
            ControlKind::Label
        } else {
            ControlKind::Control
        };
        let f = control(&mut s.ui, kind, layer, [100 + i * 150, 200, 90, 40]);
        s.ui.control_set_stretch_mode(f, 3);
        if kind == ControlKind::Label {
            s.ui.set_text(f, Some(TEXTS[i as usize].to_string()));
        } else {
            s.ui.control_set_background(f, Background::Did(IMAGES[1]));
        }
        s.floaters.push(f);
    }
    // Layer content drawn after the floaters and over them: the parts inside their
    // coverage regions go to the layer's canvas above them (Decision 1.7).
    let caption = control(&mut s.ui, ControlKind::Label, layer, [80, 190, 420, 60]);
    s.ui.set_text(caption, Some(TEXTS[3].to_string()));
    let veil = control(&mut s.ui, ControlKind::Control, layer, [90, 215, 400, 20]);
    s.ui.control_set_back_colour(veil, Some([0.2, 0.4, 0.8, 0.5]));
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
    LayerOpacity(u8),
    ContainerMove(usize, i32, i32),
    ContainerVisible(usize, bool),
    NestedVisible(usize, bool),
    FloatMove(usize, i32, i32),
    FloatSize(usize, i32, i32),
    FloatScale(usize, u8),
    FloatText(usize, usize),
    FloatOpacity(usize, u8),
    FloatVisible(usize, bool),
    FloatParent(usize, usize),
    FloatStretch(usize, i32),
    FloatSizePow2(usize, u32, u32),
    NestedBackground(usize, usize),
    ContainerZ(usize, i32),
    ContainerParent(usize, usize),
    WindowZ(usize, i32),
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
        any::<u8>().prop_map(Edit::LayerOpacity),
        (0..3usize, -9..10i32, -9..10i32).prop_map(|(i, x, y)| Edit::ContainerMove(i, x, y)),
        (0..3usize, any::<bool>()).prop_map(|(i, v)| Edit::ContainerVisible(i, v)),
        (0..2usize, any::<bool>()).prop_map(|(i, v)| Edit::NestedVisible(i, v)),
        (0..3usize, -9..10i32, -9..10i32).prop_map(|(i, x, y)| Edit::FloatMove(i, x, y)),
        (0..3usize, 0..120i32, 0..60i32).prop_map(|(i, w, h)| Edit::FloatSize(i, w, h)),
        (0..3usize, any::<u8>()).prop_map(|(i, c)| Edit::FloatScale(i, c)),
        (0..3usize, 0..TEXTS.len()).prop_map(|(i, t)| Edit::FloatText(i, t)),
        (0..3usize, any::<u8>()).prop_map(|(i, o)| Edit::FloatOpacity(i, o)),
        (0..3usize, any::<bool>()).prop_map(|(i, v)| Edit::FloatVisible(i, v)),
        (0..3usize, 0..5usize).prop_map(|(i, p)| Edit::FloatParent(i, p)),
        (0..3usize, 0..4i32).prop_map(|(i, m)| Edit::FloatStretch(i, m)),
        (0..3usize, 3..7u32, 3..7u32).prop_map(|(i, w, h)| Edit::FloatSizePow2(i, w, h)),
        (0..2usize, 0..4usize).prop_map(|(i, b)| Edit::NestedBackground(i, b)),
        (0..3usize, -2..3i32).prop_map(|(i, z)| Edit::ContainerZ(i, z)),
        (0..3usize, 0..3usize).prop_map(|(i, w)| Edit::ContainerParent(i, w)),
        (0..4usize, -2..3i32).prop_map(|(i, z)| Edit::WindowZ(i, z)),
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
        Edit::LayerOpacity(o) => ui.control_set_opacity(s.layer, f32::from(o) / 255.0),
        Edit::ContainerMove(i, dx, dy) => {
            let k = s.containers[i];
            let (x, y) = ui.control_position(k);
            ui.control_set_position(k, x + dx, y + dy);
        }
        Edit::ContainerVisible(i, v) => ui.control_set_visible(s.containers[i], v),
        Edit::NestedVisible(i, v) => ui.control_set_visible(s.nested[i], v),
        Edit::FloatMove(i, dx, dy) => {
            let k = s.floaters[i];
            let (x, y) = ui.control_position(k);
            ui.control_set_position(k, x + dx, y + dy);
        }
        Edit::FloatSize(i, w, h) => ui.control_set_size(s.floaters[i], w, h),
        Edit::FloatScale(i, c) => {
            // 0 clears the node scale; otherwise 0.25 to about 2.25.
            let v = 0.25 + f32::from(c) / 128.0;
            let scale = (c != 0).then_some([v, v * 0.9, 1.0]);
            ui.set_node_scale(s.floaters[i], scale);
        }
        Edit::FloatText(i, t) => ui.set_text(s.floaters[i], Some(TEXTS[t].to_string())),
        Edit::FloatOpacity(i, o) => ui.control_set_opacity(s.floaters[i], f32::from(o) / 255.0),
        Edit::FloatVisible(i, v) => ui.control_set_visible(s.floaters[i], v),
        Edit::FloatParent(i, p) => {
            // The layer, a window, a container, or no parent at all.
            let parent = match p {
                0 => Some(s.layer),
                1 => Some(s.windows[0]),
                2 => Some(s.containers[1]),
                3 => Some(s.nested[1]),
                _ => None,
            };
            let _ = ui.control_set_parent(s.floaters[i], parent);
        }
        Edit::FloatStretch(i, m) => ui.control_set_stretch_mode(s.floaters[i], m),
        // Power-of-two sides (8 to 64), which a Tile-mode owner needs for tile addressing.
        Edit::FloatSizePow2(i, w, h) => ui.control_set_size(s.floaters[i], 1 << w, 1 << h),
        Edit::NestedBackground(i, b) => {
            let bg = IMAGES
                .get(b)
                .map_or(Background::None, |d| Background::Did(*d));
            ui.control_set_background(s.nested[i], bg);
        }
        Edit::ContainerZ(i, z) => ui.control_set_z_order(s.containers[i], z),
        Edit::ContainerParent(i, w) => {
            let _ = ui.control_set_parent(s.containers[i], Some(s.windows[w]));
        }
        Edit::WindowZ(i, z) => {
            let k = if i < 3 { s.windows[i] } else { s.layer };
            ui.control_set_z_order(k, z);
        }
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
        same_frames(None, &frames)?;
    }

    #[test]
    fn incremental_frames_equal_full_frames_under_a_ui_screen(
        frames in prop::collection::vec(prop::collection::vec(edit(), 0..4), 1..24),
        screen in 0..SCREENS.len(),
    ) {
        same_frames(SCREENS.get(screen).copied(), &frames)?;
    }

    #[test]
    fn incremental_frames_equal_full_frames_in_a_band_budget(
        frames in prop::collection::vec(prop::collection::vec(edit(), 0..4), 1..24),
        bands in 1..4u32,
        screen in 0..=SCREENS.len(),
    ) {
        same_frames_in(SCREENS.get(screen).copied(), Some(bands), &frames)?;
    }
}

/// Plays `frames` on an incremental and a reference UI and compares every frame.
fn same_frames(screen: Screen, frames: &[Vec<Edit>]) -> Result<(), TestCaseError> {
    same_frames_in(screen, None, frames)
}

/// [`same_frames`] with every frame's paint limited to `bands` bands (`None`: unlimited),
/// so that jobs and generations span frames.
fn same_frames_in(
    screen: Screen,
    bands: Option<u32>,
    frames: &[Vec<Edit>],
) -> Result<(), TestCaseError> {
    let mut inc = scene_with(false, screen);
    let mut full = scene_with(true, screen);
    let (mut ti, mut tf) = (Textures::default(), Textures::default());
    let mut t = Duration::ZERO;
    let mut step = |inc: &mut Scene, full: &mut Scene, ti: &mut Textures, tf: &mut Textures| {
        t += Duration::from_millis(16);
        let (a, b) = match bands {
            Some(n) => (
                inc.ui.paint(t, &mut BandBudget(n)),
                full.ui.paint(t, &mut BandBudget(n)),
            ),
            None => (
                inc.ui.paint(t, &mut Unlimited),
                full.ui.paint(t, &mut Unlimited),
            ),
        };
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
            // The reference repaints everything when images arrive, which takes more
            // bands: in a band budget the two would differ until both settle. Under a UI
            // screen it would also redraw canvases that both UIs leave stale after a move
            // by a fraction of a pixel (the ignored test
            // `a_nested_surface_moved_by_a_fraction_of_a_pixel_is_repainted`).
            if (bands.is_some() || screen.is_some()) && matches!(e, Edit::ImagesArrived) {
                continue;
            }
            apply(&mut inc, e);
            apply(&mut full, e);
        }
        let (a, b) = step(&mut inc, &mut full, &mut ti, &mut tf);
        prop_assert_eq!(a, b, "composition of frame {}", n);
        prop_assert!(ti.front() == tf.front(), "textures after frame {}", n);
    }
    Ok(())
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

/// A window shown while its background image is loading, painted on a clock that does not
/// advance (headless screens run many frames at one checkpoint): its first paint is
/// withheld for the image (Decision 1.8), and once the image arrives the first paint is
/// redone with it and drawn, without waiting for the 100 ms of U4 (c) to run out on the
/// clock. Before the fix the withheld first paint of the new group's generation never saw
/// the arrival and the UI never settled.
#[test]
fn a_first_paint_waiting_for_an_image_is_drawn_when_it_arrives_on_a_still_clock() {
    let arrived = Arc::new(AtomicBool::new(false));
    let env = LoadingEnv {
        inner: env(),
        arrived: Arc::clone(&arrived),
    };
    let mut ui = Ui::new(Box::new(env), tessera_ui::Vocab::default(), (320, 200));
    let win = ui.create_control(ControlKind::Window, 1).expect("window");
    ui.control_set_position(win, 10, 10);
    ui.control_set_size(win, 120, 60);
    ui.control_set_visible(win, true);
    let pic = control(&mut ui, ControlKind::Control, win, [10, 10, 64, 40]);
    ui.control_set_background(pic, Background::Did(IMAGES[2]));
    let mut tex = Textures::default();
    let t = Duration::from_millis(16);
    // One band per frame, so the first paint spans frames (as large panels do in their
    // slice) and its snapshot's pending image is known before it completes.
    for _ in 0..8 {
        let f = ui.paint(t, &mut BandBudget(1));
        tex.apply(&f.ops);
        assert_eq!(f.stats.first_paint_restarts, 0);
    }
    assert!(!ui.paint_settled(), "withheld while the image loads");
    let before = tex.front();
    arrived.store(true, Ordering::Relaxed);
    ui.images_arrived();
    let mut restarts = 0;
    let mut frames = 0;
    while !ui.paint_settled() {
        let f = ui.paint(t, &mut BandBudget(1));
        tex.apply(&f.ops);
        restarts += f.stats.first_paint_restarts;
        frames += 1;
        assert!(
            frames < 64,
            "settles once the image is in, on the same clock"
        );
    }
    assert_eq!(restarts, 1, "the withheld first paint started again once");
    let after = tex.front();
    assert_ne!(before, after, "the image is in the drawn picture");
    assert!(
        after.iter().any(|(_, _, px)| px.iter().any(|p| *p != 0)),
        "the drawn picture has content"
    );
}

/// Plays fixed edit sequences that the random property may take long to find: a subtree
/// that holds a surface moves among siblings that hold surfaces (the composition order of
/// the nested entries changes), and a surface owner moves and grows in its layer.
#[test]
fn fixed_sequences_equal_full_frames() {
    let seqs: Vec<Vec<Vec<Edit>>> = vec![
        // A floater next to the container that holds the stretch control, then the
        // container goes above it, below it, and the window above the layer.
        vec![
            vec![Edit::FloatParent(0, 1)],
            vec![Edit::ContainerZ(0, 2)],
            vec![Edit::ContainerZ(0, -2)],
            vec![Edit::WindowZ(0, 2)],
            vec![Edit::FloatParent(0, 0)],
        ],
        // A floater into the child window and back, and the window's subtree moved.
        vec![
            vec![Edit::FloatParent(1, 3)],
            vec![Edit::WindowZ(1, 2)],
            vec![Edit::ContainerParent(0, 1)],
            vec![Edit::ContainerZ(0, 1)],
            vec![Edit::FloatParent(1, 4)],
            vec![Edit::FloatParent(1, 0)],
        ],
        // A floater set to Tile mode at a size without power-of-two sides (no tile
        // addressing), then resized to power-of-two sides in a later frame: the full pass
        // decides a Tile surface, which the light pass must leave to it; then back.
        vec![
            vec![Edit::FloatStretch(0, 2)],
            vec![Edit::FloatSizePow2(0, 3, 3)],
            vec![],
            vec![Edit::FloatSizePow2(0, 5, 4)],
            vec![Edit::FloatSize(0, 90, 40)],
            vec![Edit::FloatSizePow2(0, 6, 6)],
            vec![Edit::FloatStretch(0, 3)],
            vec![],
        ],
        // Floating text: moves, grows, scales and fades every frame.
        (0..12)
            .map(|f| {
                vec![
                    Edit::FloatMove(0, 1, -2),
                    Edit::FloatScale(1, 100 + f * 9),
                    Edit::FloatSize(2, 60 + i32::from(f) * 3, 30 + i32::from(f)),
                    Edit::FloatOpacity(0, 255 - f * 20),
                    Edit::FloatText(1, usize::from(f) % TEXTS.len()),
                ]
            })
            .collect(),
    ];
    for screen in [None, Some(SCREENS[0]), Some(SCREENS[1]), Some(SCREENS[2])] {
        for seq in &seqs {
            same_frames(screen, seq).expect("the frames of the full pass");
            same_frames_in(screen, Some(1), seq).expect("the frames of the full pass, banded");
        }
    }
}

/// A frame in which nothing changed runs no surface pass, paints and uploads nothing, and
/// gives the composition list of the frame before; the floating-text edits of a steady
/// HUD run the light pass, not the full one (the busiest-scenario regression of
/// `docs/perf/p8-busiest.md`, "ui-perf").
#[test]
fn a_frame_without_change_paints_nothing_and_steady_hud_changes_stay_light() {
    let mut s = scene_with(false, Some(SCREENS[0]));
    s.arrived.store(true, Ordering::Relaxed);
    let mut t = Duration::ZERO;
    let mut frames = 0;
    loop {
        t += Duration::from_millis(16);
        let _ = s.ui.paint(t, &mut Unlimited);
        frames += 1;
        if s.ui.paint_settled() {
            break;
        }
        assert!(frames < 256, "the first paints settle");
    }
    t += Duration::from_millis(16);
    let before = s.ui.paint(t, &mut Unlimited).composition;
    let passes = s.ui.surface_passes();
    for _ in 0..30 {
        t += Duration::from_millis(16);
        let f = s.ui.paint(t, &mut Unlimited);
        assert_eq!(f.stats.bands, 0, "no band painted without a change");
        assert_eq!(f.stats.jobs_started, 0, "no job without a change");
        assert_eq!(f.stats.upload_bytes, 0, "nothing uploaded without a change");
        assert!(f.ops.is_empty(), "no GPU operation without a change");
        assert_eq!(f.composition, before, "the same composition");
    }
    assert_eq!(
        s.ui.surface_passes(),
        passes,
        "no surface pass without a change"
    );
    // Floating text in its layer: moving, growing, rescaling, new text and opacity, while
    // the layer itself (a recorded ancestor of the floaters) fades.
    for f in 0..40u8 {
        for e in [
            Edit::LayerOpacity(255 - f),
            Edit::FloatMove(0, 1, -1),
            Edit::FloatScale(1, 60 + f),
            Edit::FloatSize(2, 60 + i32::from(f), 30),
            Edit::FloatOpacity(0, 250 - f * 4),
            Edit::FloatText(1, usize::from(f) % TEXTS.len()),
            Edit::NestedBackground(0, usize::from(f) % 4),
        ] {
            apply(&mut s, &e);
        }
        t += Duration::from_millis(16);
        let _ = s.ui.paint(t, &mut Unlimited);
    }
    let after = s.ui.surface_passes();
    assert_eq!(
        after.full, passes.full,
        "steady HUD changes run no full pass"
    );
    assert_eq!(
        after.light,
        passes.light + 40,
        "each changed frame runs the light pass"
    );
}

/// A nested surface owner moved by a fraction of a physical pixel under a UI screen (here
/// *K* = 0.625), with its canvas size unchanged: neither the full pass nor the light pass
/// repaints its canvas, although its elements' canvas boxes (`ui-scaling.md` §6.2: physical
/// box minus the canvas origin) may move by a pixel. Both UIs keep the same stale canvas
/// until images arrive, when the reference repaints everything. Whether such a surface must
/// be repainted is an open question of the package (decision 0015 / `ui-scaling.md` §6.2);
/// the UI-screen properties leave image arrivals out until it is answered.
#[test]
#[ignore = "open question: repaint of a surface whose canvas origin moves by a fraction"]
fn a_nested_surface_moved_by_a_fraction_of_a_pixel_is_repainted() {
    let seq = vec![
        vec![Edit::MoveNested(0, -4, 0)],
        vec![Edit::WindowMove(0, -4, 6), Edit::StretchSize(39)],
        vec![Edit::ImagesArrived],
    ];
    same_frames(Some(SCREENS[0]), &seq).expect("the frames of the full pass");
}

/// The full surface pass over `n` sibling windows under the screen root, forced by
/// toggling one window's visibility: the least time of a few frames.
fn full_pass_over_siblings(n: usize) -> Duration {
    let mut ui = Ui::new(Box::new(env()), tessera_ui::Vocab::default(), (640, 480));
    let mut first = None;
    for i in 0..n {
        let w = ui.create_control(ControlKind::Window, 1).expect("window");
        ui.control_set_position(w, (i % 60) as i32 * 10, (i / 60 % 40) as i32 * 10);
        ui.control_set_size(w, 8, 8);
        ui.control_set_visible(w, true);
        first.get_or_insert(w);
    }
    let first = first.expect("a window");
    let mut t = Duration::ZERO;
    for _ in 0..4 {
        t += Duration::from_millis(16);
        let _ = ui.paint(t, &mut Unlimited);
    }
    let mut best = Duration::MAX;
    for f in 0..6 {
        ui.control_set_visible(first, f % 2 == 1);
        let full = ui.surface_passes().full;
        t += Duration::from_millis(16);
        let start = std::time::Instant::now();
        let _ = ui.paint(t, &mut Unlimited);
        best = best.min(start.elapsed());
        assert_eq!(ui.surface_passes().full, full + 1, "a full pass ran");
    }
    best
}

/// The full surface pass grows linearly with the number of sibling surface owners: a
/// plugin may create thousands of windows under the screen root, and a pass that searched
/// each owner's position among its siblings was quadratic in them. Four times the windows
/// must cost well under the sixteen times of a quadratic pass (about 5 against 10 measured
/// in a release build). The times are only compared in an optimised build: in a debug
/// build the linear parts of the pass hide the difference at sizes a test can afford.
#[test]
fn the_full_pass_is_linear_in_sibling_windows() {
    let (n, check) = if cfg!(debug_assertions) {
        (300, false)
    } else {
        (1_500, true)
    };
    let small = full_pass_over_siblings(n);
    let large = full_pass_over_siblings(4 * n);
    let ratio = large.as_secs_f64() / small.as_secs_f64().max(1e-9);
    eprintln!(
        "full pass: {n} windows {small:?}, {} windows {large:?}, ratio {ratio:.2}",
        4 * n
    );
    if check {
        assert!(
            ratio < 8.0,
            "{n} windows {small:?}, {} windows {large:?}: ratio {ratio:.2}",
            4 * n
        );
    }
}
