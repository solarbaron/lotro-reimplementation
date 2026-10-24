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

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use proptest::prelude::*;
use tessera_formats::Did;
use tessera_formats::ui_layout::UiLayout;
use tessera_material::ui_image::{PixelLayout, UiImage};
use tessera_ui::paint::{
    BandBudget, CompositionList, Entry, EntryTexture, GpuOp, Layer, Rect, Slot, TexKey, Unlimited,
};
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
    /// The frame comes 160 ms after the one before: every geometry preview that held since
    /// then is due for its settle repaint (`ui-scaling.md` §6.1 item 4).
    Rest,
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
        Just(Edit::Rest),
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
        Edit::Rest | Edit::Nothing => {}
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

    /// `ui-scaling.md` I20 (b), (d): after any edits, 150 ms at rest and the images in, every
    /// canvas of the incremental UI is the canvas a fresh paint at the final geometry gives.
    #[test]
    fn settled_canvases_equal_a_fresh_paint(
        frames in prop::collection::vec(prop::collection::vec(edit(), 0..4), 1..16),
        screen in 0..=SCREENS.len(),
    ) {
        let screen = SCREENS.get(screen).copied();
        let mut frames = frames;
        frames.push(vec![Edit::ImagesArrived]);
        let (mut inc, t) = same_frames_in(screen, None, &frames)?;
        settle(&mut inc, t);
        same_canvases(&inc, &fresh(screen, &frames))?;
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

/// The reference painter of a frame-by-frame comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reference {
    /// The full surface pass in every frame (`Ui::set_reference_paint`), with the damage it
    /// finds.
    FullPass,
    /// The full surface pass and a full repaint of every surface in every frame
    /// (`Ui::invalidate_paint` before each paint): `ui-scaling.md` I20 (d) and `render.md`
    /// R14 item 4. Both UIs get an upload cap that a full repaint fits in, so that neither
    /// defers uploads.
    Repaint,
}

proptest! {
    // A full repaint of every surface in every frame is slow: fewer cases.
    #![proptest_config(ProptestConfig { cases: 24, ..ProptestConfig::default() })]

    /// `ui-scaling.md` I20 (d), `render.md` R14 item 4: frame by frame, the incremental UI
    /// gives the frames of a full repaint of every surface in every frame, under a UI screen
    /// with non-integer *Kx*, *Ky*, through moves, node scales and rests (geometry previews
    /// and their settles).
    #[test]
    fn incremental_frames_equal_a_full_repaint_every_frame(
        frames in prop::collection::vec(prop::collection::vec(edit(), 0..4), 1..16),
        screen in 0..=SCREENS.len(),
    ) {
        same_frames_kept(SCREENS.get(screen).copied(), None, Reference::Repaint, &frames)?;
    }
}

/// A Stretch surface resized with nothing else changing (`ui-behaviour.md` §9.2.1: only its
/// stretch changes) is shown at its new size in that frame, as a full repaint of every surface
/// shows it; before, the change waited for some other change to touch the tree. Under a UI
/// screen the equal-factor stretch counts as uniform (`ui-scaling.md` §3.4): a factor of 4 is
/// past the bound of §6.1 item 5, so the surface is repainted at once.
#[test]
fn a_stretch_resize_alone_is_shown_like_a_full_repaint() {
    for screen in [None, Some(SCREENS[0])] {
        for w in [10, 39, 55] {
            same_frames_kept(
                screen,
                None,
                Reference::Repaint,
                &[vec![Edit::StretchSize(w)]],
            )
            .expect("the frames of a full repaint");
        }
    }
}

/// Plays `frames` on an incremental and a reference UI and compares every frame.
fn same_frames(screen: Screen, frames: &[Vec<Edit>]) -> Result<(), TestCaseError> {
    same_frames_in(screen, None, frames).map(|_| ())
}

/// [`same_frames`] with every frame's paint limited to `bands` bands (`None`: unlimited),
/// so that jobs and generations span frames.
fn same_frames_in(
    screen: Screen,
    bands: Option<u32>,
    frames: &[Vec<Edit>],
) -> Result<(Scene, Duration), TestCaseError> {
    same_frames_kept(screen, bands, Reference::FullPass, frames).map(|(inc, _, t)| (inc, t))
}

/// [`same_frames_in`] against `reference`, returning both UIs (incremental, reference) and the
/// clock of the last frame.
fn same_frames_kept(
    screen: Screen,
    bands: Option<u32>,
    reference: Reference,
    frames: &[Vec<Edit>],
) -> Result<(Scene, Scene, Duration), TestCaseError> {
    let mut inc = scene_with(false, screen);
    let mut full = scene_with(true, screen);
    if reference == Reference::Repaint {
        let limits = tessera_ui::paint::PaintLimits {
            upload_cap_bytes: 1 << 40,
            ..tessera_ui::paint::PaintLimits::default()
        };
        inc.ui.set_paint_limits(limits);
        full.ui.set_paint_limits(limits);
    }
    let (mut ti, mut tf) = (Textures::default(), Textures::default());
    let mut t = Duration::ZERO;
    let mut step =
        |inc: &mut Scene, full: &mut Scene, ti: &mut Textures, tf: &mut Textures, rest: bool| {
            t += Duration::from_millis(if rest { 160 } else { 16 });
            if reference == Reference::Repaint {
                full.ui.invalidate_paint();
            }
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
        let (a, b) = step(&mut inc, &mut full, &mut ti, &mut tf, false);
        prop_assert_eq!(a, b);
    }
    prop_assert_eq!(ti.front(), tf.front());
    for (n, edits) in frames.iter().enumerate() {
        for e in edits {
            // The reference repaints everything when images arrive, which takes more
            // bands: in a band budget the two would differ until both settle. (Under a UI
            // screen both paint a previewed surface at its held geometry, `ui-scaling.md`
            // §6.1 item 2, so the arrival is played there.)
            if bands.is_some() && matches!(e, Edit::ImagesArrived) {
                continue;
            }
            apply(&mut inc, e);
            apply(&mut full, e);
        }
        let rest = edits.iter().any(|e| matches!(e, Edit::Rest));
        let (a, b) = step(&mut inc, &mut full, &mut ti, &mut tf, rest);
        prop_assert_eq!(a, b, "composition of frame {}", n);
        prop_assert!(ti.front() == tf.front(), "textures after frame {}", n);
    }
    Ok((inc, full, t))
}

/// Paints `s` from clock `t` on, a frame every 16 ms, until it settles (every geometry
/// preview included, `ui-scaling.md` §6.1 item 4); returns the clock.
fn settle(s: &mut Scene, mut t: Duration) -> Duration {
    for _ in 0..256 {
        t += Duration::from_millis(16);
        let _ = s.ui.paint(t, &mut Unlimited);
        if s.ui.paint_settled() && !s.ui.geometry_preview_pending() {
            return t;
        }
    }
    panic!("the UI settles");
}

/// The scene after `frames` played without painting between them (images loaded), painted
/// until it settles: every canvas a fresh paint at the final geometry.
fn fresh(screen: Screen, frames: &[Vec<Edit>]) -> Scene {
    let mut s = scene_with(false, screen);
    s.arrived.store(true, Ordering::Relaxed);
    s.ui.images_arrived();
    for e in frames.iter().flatten() {
        apply(&mut s, e);
    }
    settle(&mut s, Duration::ZERO);
    s
}

/// Every surface owner of the scene that the edits may touch.
fn owners(s: &Scene) -> Vec<ElementKey> {
    let mut v = s.windows.clone();
    v.push(s.layer);
    v.extend(&s.nested);
    v.extend(&s.floaters);
    v.extend(&s.containers);
    v.extend(&s.controls);
    v
}

/// Every canvas of each surface of `a` equals `b`'s (size, the canvas below and each canvas
/// above a nested entry, every pixel; [`canvas_of`]): `ui-scaling.md` I20 (b), a settled
/// canvas is byte-identical to a fresh paint at the current geometry.
fn same_canvases(a: &Scene, b: &Scene) -> Result<(), TestCaseError> {
    for (ka, kb) in owners(a).into_iter().zip(owners(b)) {
        let (ca, cb) = (canvas_of_opt(&a.ui, ka), canvas_of_opt(&b.ui, kb));
        prop_assert_eq!(
            ca.as_ref().map(|c| c.0),
            cb.as_ref().map(|c| c.0),
            "canvas size of {:?}",
            ka
        );
        let (Some((_, la)), Some((_, lb))) = (ca, cb) else {
            continue;
        };
        prop_assert_eq!(
            la.keys().collect::<Vec<_>>(),
            lb.keys().collect::<Vec<_>>(),
            "canvases of {:?}",
            ka
        );
        for (layer, pa) in &la {
            let pb = lb.get(layer);
            prop_assert!(Some(pa) == pb, "pixels of {:?}, layer {:?}", ka, layer);
        }
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
        if s.ui.paint_settled() && !s.ui.geometry_preview_pending() {
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
/// *K* = 0.625), with its canvas size unchanged: its elements' canvas boxes (`ui-scaling.md`
/// §6.2: physical box minus the canvas origin) may move by a pixel, so its canvas geometry
/// changes through its phase (§6.1 geometry preview, `s6-2-phase`). It is previewed (its held
/// canvas composed 1 : 1 at its new place), images that arrive meanwhile are painted at the
/// held geometry by both painters (the reference repaints everything then), and once its
/// geometry has held for 150 ms it is repainted in full at the current geometry: the canvas a
/// fresh paint gives (I20 (b)).
#[test]
fn a_nested_surface_moved_by_a_fraction_of_a_pixel_is_repainted() {
    let mut seq = vec![
        vec![Edit::MoveNested(0, -4, 0)],
        vec![Edit::WindowMove(0, -4, 6), Edit::StretchSize(39)],
        vec![Edit::ImagesArrived],
    ];
    let (inc, full, _) = same_frames_kept(Some(SCREENS[0]), None, Reference::FullPass, &seq)
        .expect("the frames of the full pass");
    assert!(
        inc.ui.in_geometry_preview(inc.windows[0]) && full.ui.in_geometry_preview(full.windows[0]),
        "the window moved by a fraction of a pixel is previewed"
    );
    // Ten frames at rest (160 ms): the previews settle in both painters, frame by frame alike.
    seq.extend((0..10).map(|_| Vec::new()));
    let (mut inc, mut full, t) =
        same_frames_kept(Some(SCREENS[0]), None, Reference::FullPass, &seq)
            .expect("the frames of the full pass");
    let t = settle(&mut inc, t);
    settle(&mut full, t);
    assert!(!inc.ui.geometry_preview_pending() && !full.ui.geometry_preview_pending());
    let fresh = fresh(Some(SCREENS[0]), &seq);
    same_canvases(&inc, &fresh).expect("the settled canvases are a fresh paint");
    same_canvases(&full, &fresh).expect("the reference's settled canvases are a fresh paint");
    // And frame by frame like a full repaint of every surface in every frame (I20 (d)).
    same_frames_kept(Some(SCREENS[0]), None, Reference::Repaint, &seq)
        .expect("the frames of a full repaint");
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

/// The composition entry of the canvas below of `k`'s surface in `list`.
fn entry_of(ui: &Ui, list: &CompositionList, k: ElementKey) -> Option<Entry> {
    let id = ui.surface_of(k)?;
    list.entries
        .iter()
        .find(|e| {
            e.texture
                == EntryTexture::Surface {
                    surface: id,
                    layer: Layer::Below,
                }
        })
        .cloned()
}

/// A UI with images loaded, painted until it settles, under `screen`; returns the UI and its
/// clock.
fn quiet_ui(screen: Screen, build: impl FnOnce(&mut Ui)) -> (Ui, Duration) {
    let (ui, t, _) = quiet_ui_with_textures(screen, build);
    (ui, t)
}

/// [`quiet_ui`] with a model of its UI textures.
fn quiet_ui_with_textures(screen: Screen, build: impl FnOnce(&mut Ui)) -> (Ui, Duration, Textures) {
    let mut tex = Textures::default();
    let env = LoadingEnv {
        inner: env(),
        arrived: Arc::new(AtomicBool::new(true)),
    };
    let mut ui = Ui::new(Box::new(env), tessera_ui::Vocab::default(), (640, 480));
    if let Some((wp, hp, se)) = screen {
        let f = tessera_ui::scaling::UiScreen::new(wp, hp, se).expect("screen");
        ui.set_ui_screen(f, tessera_ui::scaling::safe_area(&f, None));
    }
    build(&mut ui);
    let mut t = Duration::ZERO;
    for _ in 0..256 {
        t += Duration::from_millis(16);
        let f = ui.paint(t, &mut Unlimited);
        tex.apply(&f.ops);
        if ui.paint_settled() && !ui.geometry_preview_pending() {
            return (ui, t, tex);
        }
    }
    panic!("the UI settles");
}

/// A layer of a surface's canvases, named the same way in two UIs built alike: the canvas
/// below, or the canvas above the nested entry whose owner is reached from the surface's owner
/// by these child indices.
type LayerName = Option<Vec<usize>>;

/// A surface's canvas size and its canvases by layer ([`canvas_of`]).
type Canvases = ([u32; 2], BTreeMap<LayerName, Vec<u32>>);

/// Every CPU canvas of `k`'s surface, row by row (decision 0015 Decision 1.7): the canvas
/// below and each canvas above a nested entry that holds a pixel (an all-transparent canvas
/// above draws what no canvas draws), with the canvas size.
fn canvas_of(ui: &Ui, k: ElementKey) -> Canvases {
    canvas_of_opt(ui, k).expect("a canvas")
}

/// [`canvas_of`], `None` when the surface has no canvas.
fn canvas_of_opt(ui: &Ui, k: ElementKey) -> Option<Canvases> {
    let size = ui.surface_canvas_size(k)?;
    let id = ui.surface_of(k)?;
    let mut out = BTreeMap::new();
    for layer in ui.surface_layers(k) {
        let name = match layer {
            Layer::Below => None,
            Layer::Above(e) => Some(path_to_surface(ui, k, e).unwrap_or_else(|| vec![usize::MAX])),
        };
        let mut px = Vec::with_capacity((size[0] * size[1]) as usize);
        for y in 0..size[1] as i32 {
            for x in 0..size[0] as i32 {
                px.push(ui.surface_layer_pixel(id, layer, x, y).unwrap_or(0));
            }
        }
        if name.is_some() && px.iter().all(|p| *p == 0) {
            continue;
        }
        out.insert(name, px);
    }
    Some((size, out))
}

/// The child indices from `from` to the element whose surface is `entry`.
fn path_to_surface(ui: &Ui, from: ElementKey, entry: u32) -> Option<Vec<usize>> {
    let mut stack = vec![(from, Vec::new())];
    while let Some((k, path)) = stack.pop() {
        if k != from && ui.surface_of(k).is_some_and(|s| s.0 == entry) {
            return Some(path);
        }
        if path.len() > 64 {
            continue;
        }
        for (i, c) in ui.children(k).into_iter().enumerate() {
            let mut p = path.clone();
            p.push(i);
            stack.push((c, p));
        }
    }
    None
}

/// A window with a picture and a label.
fn picture_window(ui: &mut Ui, x: i32, y: i32) -> ElementKey {
    let w = ui.create_control(ControlKind::Window, 1).expect("window");
    ui.control_set_position(w, x, y);
    ui.control_set_size(w, 180, 120);
    ui.control_set_visible(w, true);
    ui.control_set_background(w, Background::Did(IMAGES[0]));
    let l = control(ui, ControlKind::Label, w, [10, 10, 150, 40]);
    ui.set_text(l, Some(TEXTS[3].to_string()));
    w
}

/// `ui-scaling.md` §6.1 geometry preview and I20 (a), (b): a window moved by a fraction of a
/// physical pixel (*K* = 0.625) is not repainted in the frame of the move; its held canvas is
/// composed 1 : 1 at its new physical top-left, limited to its new physical box (each edge
/// within one pixel of it). After 150 ms at rest it is repainted once, in full, at the current
/// geometry: composed 1 : 1 onto exactly its physical box, with the canvas a fresh paint gives.
#[test]
fn a_moved_window_is_previewed_then_repainted_crisp_after_the_settle_time() {
    let screen = Some(SCREENS[0]);
    let mut w = None;
    let (mut ui, mut t) = quiet_ui(screen, |ui| w = Some(picture_window(ui, 20, 30)));
    let w = w.expect("window");
    let held = canvas_of(&ui, w);
    ui.control_set_position(w, 21, 30);
    t += Duration::from_millis(16);
    let f = ui.paint(t, &mut Unlimited);
    assert!(
        ui.in_geometry_preview(w),
        "a move by 0.625 pixels is previewed"
    );
    assert_eq!(
        (f.stats.jobs_started, f.stats.bands, f.stats.upload_bytes),
        (0, 0, 0),
        "the move paints and uploads nothing"
    );
    let phys = ui.physical_box(w);
    let e = entry_of(&ui, &f.composition, w).expect("an entry");
    assert_eq!(
        (e.dest.x0, e.dest.y0),
        (phys.x0, phys.y0),
        "at the new top-left"
    );
    assert_eq!(
        e.canvas,
        [e.dest.width(), e.dest.height()],
        "composed 1 : 1"
    );
    assert!(
        e.dest.x1 <= phys.x1 && e.dest.y1 <= phys.y1 && phys.x1 - e.dest.x1 <= 1,
        "limited to the physical box, edges within a pixel: {:?} in {phys:?}",
        e.dest
    );
    assert_eq!(canvas_of(&ui, w), held, "the held canvas");
    // 144 ms more at rest: still previewed; at 160 ms the settle repaint.
    for _ in 0..9 {
        t += Duration::from_millis(16);
        let f = ui.paint(t, &mut Unlimited);
        assert_eq!(
            f.stats.jobs_started, 0,
            "nothing painted before the settle time"
        );
        assert!(ui.in_geometry_preview(w));
    }
    t += Duration::from_millis(16);
    let f = ui.paint(t, &mut Unlimited);
    assert!(!ui.in_geometry_preview(w), "the preview ends after 150 ms");
    assert!(f.stats.jobs_started >= 1, "one repaint");
    for _ in 0..64 {
        if ui.paint_settled() && !ui.geometry_preview_pending() {
            break;
        }
        t += Duration::from_millis(16);
        let _ = ui.paint(t, &mut Unlimited);
    }
    t += Duration::from_millis(16);
    let f = ui.paint(t, &mut Unlimited);
    let e = entry_of(&ui, &f.composition, w).expect("an entry");
    assert_eq!(e.dest, phys, "composed onto the physical box");
    assert_eq!(e.canvas, [phys.width(), phys.height()], "1 : 1");
    let mut w2 = None;
    let (fresh, _) = quiet_ui(screen, |ui| w2 = Some(picture_window(ui, 21, 30)));
    assert!(
        canvas_of(&ui, w) == canvas_of(&fresh, w2.expect("window")),
        "the settled canvas is a fresh paint at the current geometry (I20 (b))"
    );
}

/// A canvas-resize label in a transparent screen-sized layer (floating combat text).
fn layer_with_floater(ui: &mut Ui, after: bool) -> (ElementKey, ElementKey, Option<ElementKey>) {
    let layer = ui.create_control(ControlKind::Window, 1).expect("window");
    ui.control_set_position(layer, 0, 0);
    ui.control_set_size(layer, 640, 480);
    ui.control_set_visible(layer, true);
    // Drawn before the floater: stays in the layer's canvas below wherever it moves.
    let under = control(ui, ControlKind::Control, layer, [100, 100, 300, 200]);
    ui.control_set_back_colour(under, Some([0.2, 0.4, 0.8, 0.5]));
    let f = control(ui, ControlKind::Label, layer, [200, 200, 90, 40]);
    ui.control_set_stretch_mode(f, 3);
    ui.set_text(f, Some(TEXTS[0].to_string()));
    // Optionally drawn after it, over its path.
    let over = after.then(|| {
        let o = control(ui, ControlKind::Control, layer, [180, 150, 60, 30]);
        ui.control_set_back_colour(o, Some([0.9, 0.1, 0.1, 0.5]));
        o
    });
    (layer, f, over)
}

/// `render.md` §6.12 R13: a nested entry that moves under a parent that draws nothing after it
/// at its old or new coverage region costs its parent no damage and no generation membership;
/// with nothing to repaint the frame paints nothing (an empty generation swaps at once, R13
/// item 4). Where the parent draws something after the entry over the region, the parent is
/// repainted there. Incremental and reference painting give the same frames, and the result is
/// a fresh paint.
#[test]
fn a_parent_takes_no_damage_where_nothing_is_drawn_after_a_moved_nested_entry() {
    for screen in [None, Some(SCREENS[1])] {
        // Nothing drawn after the floater.
        let mut keys = None;
        let (mut ui, mut t) = quiet_ui(screen, |ui| keys = Some(layer_with_floater(ui, false)));
        let (layer, f, _) = keys.expect("scene");
        for i in 0..6 {
            ui.control_set_position(f, 200 + 2 * i, 200 - 2 * i);
            t += Duration::from_millis(16);
            let fr = ui.paint(t, &mut Unlimited);
            assert_eq!(
                (fr.stats.jobs_started, fr.stats.bands, fr.stats.upload_bytes),
                (0, 0, 0),
                "frame {i} under {screen:?}: the move repaints nothing"
            );
        }
        for _ in 0..64 {
            if ui.paint_settled() && !ui.geometry_preview_pending() {
                break;
            }
            t += Duration::from_millis(16);
            let _ = ui.paint(t, &mut Unlimited);
        }
        let mut k2 = None;
        let (fresh, _) = quiet_ui(screen, |ui| {
            let k = layer_with_floater(ui, false);
            ui.control_set_position(k.1, 210, 190);
            k2 = Some(k);
        });
        let (fl, ff, _) = k2.expect("scene");
        assert!(
            canvas_of(&ui, layer) == canvas_of(&fresh, fl),
            "the layer is a fresh paint"
        );
        assert!(
            canvas_of(&ui, f) == canvas_of(&fresh, ff),
            "the floater is a fresh paint"
        );
        // Something drawn after it over its path: the layer is repainted there.
        let mut keys = None;
        let (mut ui, mut t) = quiet_ui(screen, |ui| keys = Some(layer_with_floater(ui, true)));
        let (_, f, _) = keys.expect("scene");
        ui.control_set_position(f, 200, 160);
        t += Duration::from_millis(16);
        let fr = ui.paint(t, &mut Unlimited);
        assert!(
            fr.stats.jobs_started >= 1,
            "the layer's split changes under {screen:?}"
        );
    }
    // Frame by frame like the reference, both with and without content over the path.
    for after in [false, true] {
        for screen in [None, Some(SCREENS[0]), Some(SCREENS[1])] {
            let mut uis: Vec<(Ui, ElementKey)> = [false, true]
                .into_iter()
                .map(|reference| {
                    let mut f = None;
                    let (mut ui, _) =
                        quiet_ui(screen, |ui| f = Some(layer_with_floater(ui, after).1));
                    ui.set_reference_paint(reference);
                    (ui, f.expect("floater"))
                })
                .collect();
            let mut tex = [Textures::default(), Textures::default()];
            let mut t = Duration::from_secs(10);
            for step in 0..30 {
                t += Duration::from_millis(if step % 7 == 6 { 160 } else { 16 });
                let mut comps = Vec::new();
                for ((ui, f), tx) in uis.iter_mut().zip(tex.iter_mut()) {
                    if step % 7 != 6 {
                        let (x, y) = ui.control_position(*f);
                        ui.control_set_position(*f, x + 3, y - 5);
                        ui.set_node_scale(*f, Some([1.0 + step as f32 / 40.0; 3]));
                    }
                    let fr = ui.paint(t, &mut Unlimited);
                    tx.apply(&fr.ops);
                    comps.push(fr.composition);
                }
                assert_eq!(comps[0], comps[1], "composition of frame {step}");
                assert!(
                    tex[0].front() == tex[1].front(),
                    "textures after frame {step}"
                );
            }
        }
    }
}

/// `ui-scaling.md` §6.1 items 3 and 5, I20 (c): a node-scale change (floating text that grows)
/// is composed GPU-scaled from the held canvas onto the current physical box, with no paint,
/// while the physical box stays within a factor of 2 of the held canvas; beyond it the surface
/// is repainted at once at the current geometry, and the preview goes on from that picture.
#[test]
fn a_growing_node_scale_is_composed_scaled_within_a_factor_of_two() {
    let mut keys = None;
    let (mut ui, mut t) = quiet_ui(Some(SCREENS[1]), |ui| {
        keys = Some(layer_with_floater(ui, false))
    });
    let (_, f, _) = keys.expect("scene");
    let held = ui.surface_canvas_size(f).expect("canvas");
    let mut repaints = 0;
    for step in 1..=40 {
        ui.set_node_scale(f, Some([1.0 + step as f32 / 10.0; 3]));
        t += Duration::from_millis(16);
        let fr = ui.paint(t, &mut Unlimited);
        let phys = ui.physical_box(f);
        let e = entry_of(&ui, &fr.composition, f).expect("an entry");
        assert_eq!(e.dest, phys, "composed onto the current physical box");
        let canvas = ui.surface_canvas_size(f).expect("canvas");
        assert_eq!(e.canvas, canvas, "from the canvas it holds");
        for a in 0..2 {
            let (c, d) = (e.canvas[a], [phys.width(), phys.height()][a]);
            assert!(
                d <= 2 * c && 2 * d >= c,
                "step {step}: {d} pixels from a canvas of {c} (I20 (c))"
            );
        }
        if fr.stats.jobs_started > 0 {
            repaints += 1;
            assert_eq!(
                canvas,
                [phys.width(), phys.height()],
                "repainted at the current box"
            );
        }
    }
    assert!(ui.in_geometry_preview(f));
    // 1.1 to 5.0 from 1.0: repainted when the box passes twice the held canvas, about 1×,
    // 2×, 4×, so two or three times, never every frame.
    assert!(
        (1..=3).contains(&repaints),
        "{repaints} repaints from a held canvas of {held:?}"
    );
}

/// A surface owner's `ObjectOpacity` and its moves are applied when its canvas is composed
/// (decision 0015 Decision 1.4, 1.6: no content of its own canvas changes): fading or moving a
/// window by whole pixels starts no paint job and uploads nothing; only its composition entry
/// changes.
#[test]
fn fading_or_moving_a_window_repaints_nothing() {
    for screen in [None, Some((1024, 768, 1.0))] {
        let mut w = None;
        let (mut ui, mut t) = quiet_ui(screen, |ui| w = Some(picture_window(ui, 20, 30)));
        let w = w.expect("window");
        for i in 0..10 {
            ui.control_set_opacity(w, 1.0 - i as f32 / 20.0);
            let (x, y) = ui.control_position(w);
            ui.control_set_position(w, x + 2, y + 1);
            t += Duration::from_millis(16);
            let f = ui.paint(t, &mut Unlimited);
            assert_eq!(
                (f.stats.jobs_started, f.stats.bands, f.stats.upload_bytes),
                (0, 0, 0),
                "frame {i} under {screen:?}"
            );
            let e = entry_of(&ui, &f.composition, w).expect("an entry");
            assert!(
                (e.opacity - (1.0 - i as f32 / 20.0)).abs() < 1e-6,
                "the new opacity"
            );
            assert_eq!(e.dest, ui.physical_box(w), "at the new place");
        }
    }
}

/// `render.md` §6.12 R13 item 1 while the parent's job is in flight: the layer draws a control
/// X after the floater N; X is hidden, and the layer's content job runs over several frames
/// (one band a frame). Meanwhile N moves into X's place. The last snapshot no longer has X, but
/// the layer's shown canvases still do (X sits in its canvas below there, since N's old region
/// did not meet it). So the layer must not skip that region: N is shown at its new place only
/// once the layer's canvas there no longer holds X, never over a stale X drawn under it.
#[test]
fn a_parent_with_a_job_in_flight_does_not_skip_a_moved_nested_entry() {
    for screen in [None, Some(SCREENS[1])] {
        let mut keys = None;
        let (mut ui, mut t, mut tex) =
            quiet_ui_with_textures(screen, |ui| keys = Some(layer_with_floater(ui, true)));
        let (layer, f, over) = keys.expect("scene");
        let over = over.expect("drawn after the floater");
        let lid = ui.surface_of(layer).expect("the layer's surface");
        let below = TexKey {
            surface: lid,
            layer: Layer::Below,
            slot: Slot::Front,
        };
        // A canvas pixel inside X (and inside N's new place).
        let ob = ui.physical_box(over);
        let lb = ui.physical_box(layer);
        let (sx, sy) = ((ob.x0 + ob.x1) / 2 - lb.x0, (ob.y0 + ob.y1) / 2 - lb.y0);
        let shown = |tex: &Textures| {
            tex.tex
                .get(&below)
                .filter(|(b, _)| b.contains(sx, sy))
                .map_or(0, |(b, p)| p[Textures::at(b, sx, sy)])
        };
        let with_x = shown(&tex);
        assert_ne!(
            with_x, 0,
            "X is in the layer's canvas below under {screen:?}"
        );
        let comp = ui.paint(t, &mut Unlimited).composition;
        let old_dest = entry_of(&ui, &comp, f).expect("N's entry").dest;
        // Frame 1: X is hidden; the layer's job takes one band.
        ui.control_set_visible(over, false);
        t += Duration::from_millis(16);
        let fr = ui.paint(t, &mut BandBudget(1));
        tex.apply(&fr.ops);
        assert!(!ui.paint_settled(), "the layer's job is still running");
        // Frame 2 on: N moves into X's place, the job goes on a band a frame.
        ui.control_set_position(f, 170, 140);
        let mut moved = false;
        for step in 0..256 {
            t += Duration::from_millis(16);
            let fr = ui.paint(t, &mut BandBudget(1));
            tex.apply(&fr.ops);
            let dest = entry_of(&ui, &fr.composition, f).expect("N's entry").dest;
            if dest != old_dest {
                moved = true;
                assert_ne!(
                    shown(&tex),
                    with_x,
                    "frame {step} under {screen:?}: N is at its new place over a stale X"
                );
            }
            if ui.paint_settled() && !ui.geometry_preview_pending() {
                break;
            }
        }
        assert!(moved, "N reaches its new place under {screen:?}");
        // At rest the layer is a fresh paint of the new state.
        let mut k2 = None;
        let (fresh, _) = quiet_ui(screen, |ui| {
            let k = layer_with_floater(ui, true);
            ui.control_set_visible(k.2.expect("over"), false);
            ui.control_set_position(k.1, 170, 140);
            k2 = Some(k);
        });
        let (fl, _, _) = k2.expect("scene");
        assert!(
            canvas_of(&ui, layer) == canvas_of(&fresh, fl),
            "the layer is a fresh paint under {screen:?}"
        );
    }
}

/// A time budget with no time left after the first unit of work (a slice already used up).
struct SpentSlice;

impl tessera_ui::paint::PaintBudget for SpentSlice {
    fn exhausted(&mut self) -> bool {
        true
    }

    fn time_left(&self) -> Option<Duration> {
        Some(Duration::ZERO)
    }
}

/// `ui-scaling.md` §6.1 item 4 within the slice (§8 S7, UR §6 U4 (b)): four floating texts
/// that grew together come to rest together. Under a time budget their settle repaints, which
/// lay text out at a new content scale when their jobs start, are spread over frames: the
/// earliest due one in each frame while the slice has no time for more, the others still
/// shown as their preview. Under an unlimited budget all four settle in the frame they are due.
/// Either way the settled canvases are a fresh paint (I20 (b)).
#[test]
fn previews_due_together_settle_within_the_slice() {
    fn floaters(ui: &mut Ui) -> Vec<ElementKey> {
        let layer = ui.create_control(ControlKind::Window, 1).expect("window");
        ui.control_set_position(layer, 0, 0);
        ui.control_set_size(layer, 640, 480);
        ui.control_set_visible(layer, true);
        (0..4)
            .map(|i| {
                let f = control(ui, ControlKind::Label, layer, [60 + i * 140, 200, 90, 40]);
                ui.control_set_stretch_mode(f, 3);
                ui.set_text(f, Some(TEXTS[i as usize % TEXTS.len()].to_string()));
                f
            })
            .collect()
    }
    for spent in [true, false] {
        let mut keys = Vec::new();
        let (mut ui, mut t) = quiet_ui(Some(SCREENS[1]), |ui| keys = floaters(ui));
        for &f in &keys {
            ui.set_node_scale(f, Some([1.3, 1.3, 1.0]));
        }
        t += Duration::from_millis(16);
        let _ = ui.paint(t, &mut Unlimited);
        let previewed = |ui: &Ui| keys.iter().filter(|k| ui.in_geometry_preview(**k)).count();
        assert_eq!(previewed(&ui), 4, "all four grew: previewed");
        t += Duration::from_millis(160);
        let mut left = Vec::new();
        for _ in 0..4 {
            if spent {
                let _ = ui.paint(t, &mut SpentSlice);
            } else {
                let _ = ui.paint(t, &mut Unlimited);
            }
            left.push(previewed(&ui));
            t += Duration::from_millis(16);
        }
        let want = if spent {
            vec![3, 2, 1, 0]
        } else {
            vec![0, 0, 0, 0]
        };
        assert_eq!(
            left, want,
            "previews left after each frame (time budget: {spent})"
        );
        for _ in 0..64 {
            if ui.paint_settled() && !ui.geometry_preview_pending() {
                break;
            }
            t += Duration::from_millis(16);
            let _ = ui.paint(t, &mut Unlimited);
        }
        let mut fresh_keys = Vec::new();
        let (fresh, _) = quiet_ui(Some(SCREENS[1]), |ui| {
            fresh_keys = floaters(ui);
            for &f in &fresh_keys {
                ui.set_node_scale(f, Some([1.3, 1.3, 1.0]));
            }
        });
        for (a, b) in keys.iter().zip(&fresh_keys) {
            assert!(
                canvas_of(&ui, *a) == canvas_of(&fresh, *b),
                "a settled floater is a fresh paint (time budget: {spent})"
            );
        }
    }
}

/// The characters a text hit test finds in `rows` × `cols` of window-physical pixels.
fn hits(
    ui: &Ui,
    k: ElementKey,
    cols: std::ops::Range<i32>,
    rows: std::ops::Range<i32>,
) -> Vec<Option<usize>> {
    rows.flat_map(|y| cols.clone().map(move |x| (x, y)))
        .map(|(x, y)| ui.text_hit_test(k, x, y))
        .collect()
}

/// `ui-scaling.md` §6.4.6 during a geometry preview, item 1, and I20 (e): a window moved by one
/// UI unit at *Se* = 1.25 is previewed with the held node scales, its held canvas composed 1 : 1
/// at its new physical top-left. The label inside it moves by two pixels while the canvas moves
/// by one, so the hit test must take the pointer into the held canvas (pointer minus the
/// surface's current top-left) and look it up in the held layout: every pixel finds exactly the
/// character the held picture shows there, the one found at the same canvas pixel before.
#[test]
fn text_hit_tests_during_a_phase_preview_use_the_held_canvas() {
    let screen = Some(SCREENS[1]);
    let mut w = None;
    let (mut ui, mut t) = quiet_ui(screen, |ui| w = Some(picture_window(ui, 20, 30)));
    let w = w.expect("window");
    let label = ui.children(w)[0];
    let (b0, w0) = (ui.physical_box(label), ui.physical_box(w));
    let cols = b0.x0..b0.x1;
    let rows = b0.y0..b0.y0 + 16;
    let before = hits(&ui, label, cols.clone(), rows.clone());
    assert!(before.iter().any(Option::is_some), "the text is hit");
    ui.control_set_position(w, 21, 31);
    t += Duration::from_millis(16);
    let _ = ui.paint(t, &mut Unlimited);
    assert!(
        ui.in_geometry_preview(w),
        "a move by 1.25 pixels is previewed"
    );
    let (b1, w1) = (ui.physical_box(label), ui.physical_box(w));
    let (dx, dy) = (w1.x0 - w0.x0, w1.y0 - w0.y0);
    assert_ne!(
        (b1.x0 - b0.x0, b1.y0 - b0.y0),
        (dx, dy),
        "the label moves by another number of pixels than the canvas"
    );
    let after = hits(
        &ui,
        label,
        cols.start + dx..cols.end + dx,
        rows.start + dy..rows.end + dy,
    );
    assert_eq!(
        after, before,
        "the characters the held canvas shows (I20 (e))"
    );
    // Settled: the layout of the settle repaint, at the label's current box.
    assert!(ui.capture_settle(t, &mut Unlimited, 64, |_| {}));
    let settled = hits(&ui, label, b1.x0..b1.x1, b1.y0..b1.y0 + 16);
    let mut fresh_w = None;
    let (fresh, _) = quiet_ui(screen, |ui| fresh_w = Some(picture_window(ui, 21, 31)));
    let fl = fresh.children(fresh_w.expect("window"))[0];
    assert_eq!(
        settled,
        hits(&fresh, fl, b1.x0..b1.x1, b1.y0..b1.y0 + 16),
        "after the settle the hit test is that of a fresh paint"
    );
}

/// `ui-scaling.md` §6.4.6 during a geometry preview, item 2, and I20 (e): a floating label whose
/// node scale grows by 1.5 is previewed GPU-scaled from its held canvas. A pointer pixel of the
/// current physical box is scaled into the held canvas (pixel centre × held size ÷ current
/// size, rounded down) and looked up in the held layout, so it finds the character found before
/// the change at that held-canvas pixel; outside the current box nothing is hit.
#[test]
fn text_hit_tests_during_a_scale_preview_scale_the_pointer_into_the_held_canvas() {
    let mut keys = None;
    let (mut ui, mut t) = quiet_ui(Some(SCREENS[1]), |ui| {
        keys = Some(layer_with_floater(ui, false))
    });
    let (_, f, _) = keys.expect("scene");
    let b0 = ui.physical_box(f);
    let before = |ui: &Ui, x: i32, y: i32| ui.text_hit_test(f, x, y);
    let held: Vec<Vec<Option<usize>>> = (b0.y0..b0.y1)
        .map(|y| (b0.x0..b0.x1).map(|x| before(&ui, x, y)).collect())
        .collect();
    assert!(
        held.iter().flatten().any(Option::is_some),
        "the text is hit"
    );
    ui.set_node_scale(f, Some([1.5, 1.5, 1.0]));
    t += Duration::from_millis(16);
    let fr = ui.paint(t, &mut Unlimited);
    assert!(ui.in_geometry_preview(f));
    assert_eq!(fr.stats.jobs_started, 0, "previewed, not repainted");
    let b1 = ui.physical_box(f);
    let (wh, hh) = (i64::from(b0.width()), i64::from(b0.height()));
    let (wc, hc) = (i64::from(b1.width()), i64::from(b1.height()));
    assert_ne!((wh, hh), (wc, hc), "the box grew");
    let mut found = 0;
    for y in b1.y0..b1.y1 {
        for x in b1.x0..b1.x1 {
            // (v + ½) × Wh ÷ Wc rounded down, in integers: ⌊(2v + 1) × Wh ÷ 2Wc⌋.
            let hx = ((2 * i64::from(x - b1.x0) + 1) * wh).div_euclid(2 * wc);
            let hy = ((2 * i64::from(y - b1.y0) + 1) * hh).div_euclid(2 * hc);
            let want = held[hy as usize][hx as usize];
            assert_eq!(ui.text_hit_test(f, x, y), want, "pixel ({x}, {y})");
            found += usize::from(want.is_some());
        }
    }
    assert!(found > 0);
    assert_eq!(ui.text_hit_test(f, b1.x1, b1.y0), None, "outside the box");
    assert_eq!(
        ui.text_hit_test(f, b1.x0 - 1, b1.y0),
        None,
        "outside the box"
    );
}

/// `ui-scaling.md` §6.4.6 during a geometry preview, items 2 and 3, in a stretched surface (the
/// alternative form of §6.1): a floating label stretched by the node scale (2, 1) grows to
/// (3, 1.25). During the preview the pointer is scaled into the held canvas and then taken back
/// by the **held** *σ* = (2, 1), not the current (3, 1.25): every pixel of the current box finds
/// the character the settled picture found at that held-canvas pixel.
#[test]
fn text_hit_tests_during_a_stretched_scale_preview_use_the_held_sigma() {
    let mut keys = None;
    let (mut ui, mut t) = quiet_ui(Some(SCREENS[1]), |ui| {
        let k = layer_with_floater(ui, false);
        ui.set_node_scale(k.1, Some([2.0, 1.0, 1.0]));
        keys = Some(k);
    });
    let (_, f, _) = keys.expect("scene");
    let b0 = ui.physical_box(f);
    let held: Vec<Vec<Option<usize>>> = (b0.y0..b0.y1)
        .map(|y| (b0.x0..b0.x1).map(|x| ui.text_hit_test(f, x, y)).collect())
        .collect();
    assert!(
        held.iter().flatten().any(Option::is_some),
        "the text is hit"
    );
    ui.set_node_scale(f, Some([3.0, 1.25, 1.0]));
    t += Duration::from_millis(16);
    let fr = ui.paint(t, &mut Unlimited);
    assert!(ui.in_geometry_preview(f), "previewed");
    assert_eq!(fr.stats.jobs_started, 0, "previewed, not repainted");
    let b1 = ui.physical_box(f);
    let (wh, hh) = (i64::from(b0.width()), i64::from(b0.height()));
    let (wc, hc) = (i64::from(b1.width()), i64::from(b1.height()));
    assert!(wc > wh && hc > hh, "the box grew on both axes");
    let mut found = 0;
    for y in b1.y0..b1.y1 {
        for x in b1.x0..b1.x1 {
            let hx = ((2 * i64::from(x - b1.x0) + 1) * wh).div_euclid(2 * wc);
            let hy = ((2 * i64::from(y - b1.y0) + 1) * hh).div_euclid(2 * hc);
            let want = held[hy as usize][hx as usize];
            assert_eq!(ui.text_hit_test(f, x, y), want, "pixel ({x}, {y})");
            found += usize::from(want.is_some());
        }
    }
    assert!(found > 0);
    // Taken back by the current σ instead, a held-canvas pixel would land at 2 ÷ 3 of its
    // place in the text: the test above finds other characters then.
}

/// `ui-scaling.md` §9.3 capture settle and I20 (f): on a paint clock that does not advance,
/// `Ui::capture_settle` settles every geometry preview as if 150 ms had passed, without moving
/// the UI clock: afterwards no surface is previewed and the canvases are a fresh paint at the
/// current geometry, while a running fade keeps the opacity of the checkpoint's moment.
#[test]
fn the_capture_settle_settles_previews_on_a_held_clock() {
    let screen = Some(SCREENS[0]);
    let mut w = None;
    let (mut ui, t) = quiet_ui(screen, |ui| w = Some(picture_window(ui, 20, 30)));
    let w = w.expect("window");
    let vv = tessera_ui::Vocab::default();
    // A running fade: the element's own `AutoFade`, faded out half-way at the checkpoint.
    ui.set_property(
        w,
        vv.pids.auto_fade,
        tessera_formats::PropertyValue::Float(1.0),
    );
    ui.set_time(5.0);
    ui.set_shown(w, false);
    ui.set_time(5.5);
    ui.frame_work();
    let opacity = ui.prop(w, vv.pids.object_opacity).float();
    assert!((opacity - 0.5).abs() < 1e-3);
    ui.control_set_position(w, 21, 30);
    // The checkpoint's frame, then frames on the same paint clock: the preview stays.
    for _ in 0..4 {
        let _ = ui.paint(t, &mut Unlimited);
    }
    assert!(
        ui.in_geometry_preview(w),
        "a held clock never reaches the settle time"
    );
    let mut frames = 0;
    assert!(ui.capture_settle(t, &mut Unlimited, 64, |_| frames += 1));
    assert!(frames >= 1);
    assert!(
        !ui.geometry_preview_pending(),
        "no surface in a preview (I20 (f))"
    );
    assert!(ui.is_fading(w), "the fade has not moved");
    assert_eq!(ui.prop(w, vv.pids.object_opacity).float(), opacity);
    let mut w2 = None;
    let (fresh, _) = quiet_ui(screen, |ui| w2 = Some(picture_window(ui, 21, 30)));
    assert!(
        canvas_of(&ui, w) == canvas_of(&fresh, w2.expect("window")),
        "the settled canvas is a fresh paint at the current geometry"
    );
    // Nothing left: the capture settle paints no further frame.
    let mut more = 0;
    assert!(ui.capture_settle(t, &mut Unlimited, 64, |_| more += 1));
    assert_eq!(more, 0);
}

/// A transparent layer with a nested surface owner A that holds a nested floater B (two levels
/// of nested entries), and optionally a control drawn after A at the place B moves to.
fn layer_with_deep_floater(ui: &mut Ui, after: bool) -> (ElementKey, ElementKey, ElementKey) {
    let layer = ui.create_control(ControlKind::Window, 1).expect("window");
    ui.control_set_position(layer, 0, 0);
    ui.control_set_size(layer, 640, 480);
    ui.control_set_visible(layer, true);
    let under = control(ui, ControlKind::Control, layer, [100, 100, 300, 200]);
    ui.control_set_back_colour(under, Some([0.2, 0.4, 0.8, 0.5]));
    let a = control(ui, ControlKind::Control, layer, [150, 150, 200, 150]);
    ui.control_set_stretch_mode(a, 3);
    let b = control(ui, ControlKind::Label, a, [20, 20, 90, 40]);
    ui.control_set_stretch_mode(b, 3);
    ui.set_text(b, Some(TEXTS[0].to_string()));
    if after {
        let o = control(ui, ControlKind::Control, layer, [360, 150, 80, 80]);
        ui.control_set_back_colour(o, Some([0.9, 0.1, 0.1, 0.5]));
    }
    (layer, a, b)
}

/// `render.md` §6.12 R13 item 3 and R15 precision 3 (per parent): when the floater B reaches
/// out of its parent A's box, A's coverage region in the layer changes through the join of UR
/// Decision 1.7 step 2. Each parent whose split the move touches makes its own test: A draws
/// nothing after B, and the layer draws nothing after A at the old or new region, so neither is
/// damaged nor a member and the move paints nothing; when the layer draws a control after A
/// over B's new place, the layer is repainted. Incremental and reference painting give the
/// same frames, and the result at rest is a fresh paint.
#[test]
fn every_parent_whose_split_a_moved_entry_touches_makes_its_own_r13_test() {
    for screen in [None, Some(SCREENS[1])] {
        for after in [false, true] {
            let mut keys = None;
            let (mut ui, mut t) =
                quiet_ui(screen, |ui| keys = Some(layer_with_deep_floater(ui, after)));
            let (layer, a, b) = keys.expect("scene");
            assert!(
                ui.surface_of(a).is_some() && ui.surface_of(b).is_some(),
                "two levels of nested entries"
            );
            // B reaches out of A's box but stays partly inside it (so it keeps its entry in
            // A's list, R15 item 3); 4 units are 5 pixels at Se = 1.25.
            ui.control_set_position(b, 152, 24);
            t += Duration::from_millis(16);
            let fr = ui.paint(t, &mut Unlimited);
            if after {
                assert!(
                    fr.stats.jobs_started >= 1,
                    "the layer draws after A over B's new place ({screen:?})"
                );
            } else {
                assert_eq!(
                    (fr.stats.jobs_started, fr.stats.bands, fr.stats.upload_bytes),
                    (0, 0, 0),
                    "neither parent is damaged ({screen:?})"
                );
            }
            assert!(ui.capture_settle(t, &mut Unlimited, 256, |_| {}));
            let mut k2 = None;
            let (fresh, _) = quiet_ui(screen, |ui| {
                let k = layer_with_deep_floater(ui, after);
                ui.control_set_position(k.2, 152, 24);
                k2 = Some(k);
            });
            let (fl, fa, fb) = k2.expect("scene");
            for (x, y, what) in [(layer, fl, "layer"), (a, fa, "A"), (b, fb, "B")] {
                assert!(
                    canvas_of(&ui, x) == canvas_of(&fresh, y),
                    "{what} is a fresh paint ({screen:?}, after {after})"
                );
            }
        }
    }
    // Frame by frame like the reference.
    for after in [false, true] {
        for screen in [None, Some(SCREENS[0]), Some(SCREENS[1])] {
            let mut uis: Vec<(Ui, ElementKey)> = [false, true]
                .into_iter()
                .map(|reference| {
                    let mut b = None;
                    let (mut ui, _) =
                        quiet_ui(screen, |ui| b = Some(layer_with_deep_floater(ui, after).2));
                    ui.set_reference_paint(reference);
                    (ui, b.expect("floater"))
                })
                .collect();
            let mut tex = [Textures::default(), Textures::default()];
            let mut t = Duration::from_secs(10);
            for step in 0..30 {
                t += Duration::from_millis(if step % 7 == 6 { 160 } else { 16 });
                let mut comps = Vec::new();
                for ((ui, b), tx) in uis.iter_mut().zip(tex.iter_mut()) {
                    if step % 7 != 6 {
                        let (x, y) = ui.control_position(*b);
                        ui.control_set_position(*b, x + 11, y + 3);
                    }
                    let fr = ui.paint(t, &mut Unlimited);
                    tx.apply(&fr.ops);
                    comps.push(fr.composition);
                }
                assert_eq!(comps[0], comps[1], "composition of frame {step}");
                assert!(
                    tex[0].front() == tex[1].front(),
                    "textures after frame {step}"
                );
            }
        }
    }
}
