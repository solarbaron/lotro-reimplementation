// SPDX-License-Identifier: GPL-3.0-or-later
//! Test support of the P5 tests: an environment that serves synthetic layouts and
//! generated art and shapes every text with Tessera's bundled font (`fonts.md` §11), click
//! events, painting a window into an RGBA image, and the committed-golden check of
//! decision 0031 tier 1. All values are invented; no install file is opened.
#![allow(dead_code)]

#[path = "../ui_common/mod.rs"]
pub mod ui_common;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tessera_formats::did::Did;
use tessera_formats::property::PropertyValue;
use tessera_formats::ui_layout::UiLayout;
use tessera_material::ui_image::{PixelLayout, UiImage};
use tessera_ui::events::{Message, MsgArg};
use tessera_ui::game::mapfallback::P5Art;
use tessera_ui::game::mapmarks::{IconEntry, IconTable, NoteIcons};
use tessera_ui::game::npc::FallbackArt;
use tessera_ui::game::{GameVocab, NpcVocab, P5Vocab};
use tessera_ui::paint::Unlimited;
use tessera_ui::text::{
    CharListBuilder, DEFAULT_RANGES, FontFace, LayoutParams, TesseraFont, layout,
};
use tessera_ui::{
    Command, ElementKey, ImageRef, ImageState, PreparedText, TextRequest, Ui, UiEnv, UiEvent, Vocab,
};

/// First DID of the generated art.
pub const ART: u32 = 0x41FE_0000;

/// The generated map image (1,024 × 768).
pub const MAP_IMAGE: u32 = 0x41FD_0001;

/// A generated dungeon map image (400 × 300).
pub const DUNGEON_IMAGE: u32 = 0x41FD_0002;

/// A tall generated dungeon image (1,600 × 2,400, §10.3).
pub const TALL_IMAGE: u32 = 0x41FD_0003;

/// The environment: layouts by DID, generated images, text shaped with the bundled font.
pub struct TestEnv {
    pub layouts: HashMap<u32, Arc<UiLayout>>,
    pub images: HashMap<ImageRef, Arc<UiImage>>,
    face: Arc<FontFace>,
    colour_pid: u32,
    /// The (horizontal, vertical) justification property IDs the shaper reads, when set
    /// (`None`: every text is laid out top-left, as before).
    pub justify: Option<(u32, u32)>,
}

impl TestEnv {
    pub fn new() -> Self {
        let font = TesseraFont::bundled().expect("bundled font");
        let face = Arc::new(font.rasterise(12, DEFAULT_RANGES).expect("rasterise"));
        let mut env = Self {
            layouts: HashMap::new(),
            images: HashMap::new(),
            face,
            colour_pid: NpcVocab::default().pids.text_font_color,
            justify: None,
        };
        for (i, img) in generated_art().into_iter().enumerate() {
            env.images
                .insert(ImageRef::Did(ART + i as u32), Arc::new(img));
        }
        env.images
            .insert(ImageRef::Did(MAP_IMAGE), Arc::new(map_image(1024, 768)));
        env.images
            .insert(ImageRef::Did(DUNGEON_IMAGE), Arc::new(map_image(400, 300)));
        env.images.insert(
            ImageRef::Did(TALL_IMAGE),
            Arc::new(solid(1600, 2400, 0xFF20_3040)),
        );
        env
    }
}

impl UiEnv for TestEnv {
    fn layout(&mut self, did: Did) -> Option<Arc<UiLayout>> {
        self.layouts.get(&did.0).cloned()
    }

    fn image(&mut self, image: &ImageRef) -> ImageState {
        match self.images.get(image) {
            Some(i) => ImageState::Ready(Arc::clone(i)),
            None => ImageState::Failed,
        }
    }

    fn shape_text(&mut self, r: &TextRequest<'_>) -> Option<Arc<PreparedText>> {
        let colour = r
            .props
            .iter()
            .find(|(p, _)| *p == self.colour_pid)
            .and_then(|(_, v)| match v {
                PropertyValue::Colour(c) => Some(u32::from_le_bytes(*c)),
                _ => None,
            })
            .unwrap_or(0xFFE0_E0E0);
        let mut b = CharListBuilder::new();
        b.colour(colour).push_str(r.text);
        let chars = b.finish();
        let fonts = vec![Some(Arc::clone(&self.face))];
        let width = r.wrap_width.unwrap_or(r.size.0).max(1);
        let enum_of = |pid: u32| {
            r.props
                .iter()
                .find(|(p, _)| *p == pid)
                .and_then(|(_, v)| match v {
                    PropertyValue::Enum(e) => Some(*e),
                    _ => None,
                })
        };
        let mut p = LayoutParams {
            width,
            height: r.size.1.max(0),
            ..LayoutParams::default()
        };
        if let Some((h, v)) = self.justify {
            if let Some(e) = enum_of(h) {
                p.horizontal = e;
            }
            if let Some(e) = enum_of(v) {
                p.vertical = e;
            }
        }
        let l = layout(&chars, &fonts, &p, None).ok()?;
        let extent = (l.content_width, l.content_height);
        Some(Arc::new(PreparedText {
            layout: l,
            chars,
            fonts,
            outline: false,
            outline_colour: 0,
            truncated: false,
            extent,
        }))
    }
}

fn img(w: u32, h: u32, px: Vec<u32>) -> UiImage {
    UiImage::new(w, h, w, h, PixelLayout::A8R8G8B8, px).expect("image")
}

pub fn solid(w: u32, h: u32, argb: u32) -> UiImage {
    img(w, h, vec![argb; (w * h) as usize])
}

/// A vertical gradient 1 pixel wide.
fn vgrad(h: u32, top: u32, bottom: u32) -> UiImage {
    let lerp = |a: u32, b: u32, t: u32, n: u32| -> u32 {
        let mut out = 0u32;
        for s in [24, 16, 8, 0] {
            let x = (a >> s) & 0xFF;
            let y = (b >> s) & 0xFF;
            let v = (x * (n - t) + y * t) / n;
            out |= v << s;
        }
        out
    };
    img(
        1,
        h,
        (0..h).map(|y| lerp(top, bottom, y, h.max(2) - 1)).collect(),
    )
}

/// A disc of `d` pixels in `argb` on transparent, with a dark rim.
fn disc(d: u32, argb: u32) -> UiImage {
    let r = d as i32;
    let px = (0..d * d)
        .map(|i| {
            let (x, y) = ((i % d) as i32 * 2 + 1 - r, (i / d) as i32 * 2 + 1 - r);
            let q = x * x + y * y;
            if q <= (r - 3) * (r - 3) {
                argb
            } else if q <= r * r {
                0xFF10_1010
            } else {
                0
            }
        })
        .collect();
    img(d, d, px)
}

/// A ring of outer diameter `d` and width `t`.
fn ring(d: u32, t: i32, argb: u32) -> UiImage {
    let r = d as i32;
    let px = (0..d * d)
        .map(|i| {
            let (x, y) = ((i % d) as i32 * 2 + 1 - r, (i / d) as i32 * 2 + 1 - r);
            let q = x * x + y * y;
            if q <= r * r && q >= (r - 2 * t) * (r - 2 * t) {
                argb
            } else {
                0
            }
        })
        .collect();
    img(d, d, px)
}

/// An arrow of `d` pixels pointing east (the player marker and off-screen icons).
fn arrow(d: u32, argb: u32) -> UiImage {
    let px = (0..d * d)
        .map(|i| {
            let (x, y) = ((i % d) as i32, (i / d) as i32);
            let c = d as i32 / 2;
            if (y - c).abs() <= (d as i32 - 1 - x) / 2 {
                argb
            } else {
                0
            }
        })
        .collect();
    img(d, d, px)
}

/// A framed square icon.
fn icon(d: u32, edge: u32, fill: u32) -> UiImage {
    let px = (0..d * d)
        .map(|i| {
            let (x, y) = (i % d, i / d);
            if x == 0 || y == 0 || x == d - 1 || y == d - 1 || x == y {
                edge
            } else {
                fill
            }
        })
        .collect();
    img(d, d, px)
}

/// A generated map: land, a river band, a lake and a grid every 64 pixels; flat colours so
/// that the golden images compress.
pub fn map_image(w: u32, h: u32) -> UiImage {
    let px = (0..w * h)
        .map(|i| {
            let (x, y) = ((i % w) as i32, (i / w) as i32);
            let (wi, hi) = (w as i32, h as i32);
            if x % 64 == 0 || y % 64 == 0 {
                return 0xFF5C_6A44;
            }
            let river = (x - wi / 3 - (y / 4)).abs() < 10;
            let (lx, ly) = (x - 3 * wi / 4, y - hi / 3);
            let lake = lx * lx + 2 * ly * ly < (wi / 10) * (wi / 10);
            if river || lake {
                0xFF2E_5A8A
            } else if y > 2 * hi / 3 {
                0xFF6E_7A4C
            } else {
                0xFF7C_8A58
            }
        })
        .collect();
    img(w, h, px)
}

/// The generated art, by index from [`ART`].
fn generated_art() -> Vec<UiImage> {
    vec![
        solid(8, 8, 0xF01A_1C24),            // 0 window
        vgrad(24, 0xFF4A_3A22, 0xFF22_1A10), // 1 title bar
        solid(8, 8, 0xC00C_0E14),            // 2 panel
        vgrad(22, 0xFF26_2A36, 0xFF1A_1D26), // 3 row
        vgrad(22, 0xFF46_4A26, 0xFF2E_3018), // 4 row highlight
        vgrad(24, 0xFF5A_4A30, 0xFF3A_2E1C), // 5 button
        vgrad(24, 0xFF40_4040, 0xFF30_3030), // 6 button ghosted
        solid(16, 16, 0xFF8A_2A20),          // 7 close
        vgrad(23, 0xFF30_3040, 0xFF20_2028), // 8 tab
        vgrad(23, 0xFF60_5030, 0xFF40_3420), // 9 tab selected
        solid(4, 4, 0xFF08_0808),            // 10 entry
        icon(16, 0xFF70_6050, 0xFF10_1010),  // 11 slot
        disc(14, 0xFFE8_C040),               // 12 gold
        disc(14, 0xFFC8_C8D0),               // 13 silver
        disc(14, 0xFFB0_6A30),               // 14 copper
        solid(4, 4, 0x8030_2010),            // 15 map padding
        solid(4, 4, 0x00_000000),            // 16 decoration (clear)
        arrow(28, 0xFFF0_F0F0),              // 17 player marker
        solid(4, 4, 0x6020_2020),            // 18 dungeon frame
        icon(16, 0xFF40_E040, 0xFF20_6020),  // 19 check
        icon(16, 0xFF60_C0F0, 0xFF20_4060),  // 20 swift
        solid(4, 4, 0x3000_00FF),            // 21 unknown overlay
        solid(4, 4, 0x30FF_0000),            // 22 failed overlay
        solid(4, 4, 0x80_202020),            // 23 ghosted overlay
        ring(237, 8, 0xFF8A_7A50),           // 24 rim
        icon(48, 0xFFF0_C040, 0xFF60_4010),  // 25 alert
        disc(16, 0xFFFF_E000),               // 26 quest note
        disc(16, 0xFFFF_FFFF),               // 27 quest highlight
        disc(18, 0xFF40_A0F0),               // 28 waypoint
        disc(14, 0xFFE0_4040),               // 29 vendor (type bit 20)
        disc(14, 0xFFC0_60E0),               // 30 trainer (type bit 21)
        arrow(16, 0xFFE0_4040),              // 31 vendor off-screen
        icon(16, 0xFFE0_4040, 0xFF60_1010),  // 32 vendor menu image
        disc(20, 0xFF40_F0A0),               // 33 own marker icon
    ]
}

/// The art reference of index `i`.
pub fn art_ref(i: u32) -> ImageRef {
    ImageRef::Did(ART + i)
}

/// The fallback art over the generated images.
pub fn art() -> P5Art {
    let a = |i| Some(art_ref(i));
    P5Art {
        base: FallbackArt {
            window: a(0),
            title_bar: a(1),
            panel: a(2),
            row: a(3),
            row_highlight: a(4),
            button: a(5),
            button_ghosted: a(6),
            close: a(7),
            tab: a(8),
            tab_selected: a(9),
            entry: a(10),
            slot: a(11),
            coin_gold: a(12),
            coin_silver: a(13),
            coin_copper: a(14),
            ..FallbackArt::default()
        },
        map_padding: a(15),
        map_decoration: a(16),
        player_marker: a(17),
        dungeon_frame: a(18),
        check: a(19),
        swift: a(20),
        unknown: a(21),
        failed: a(22),
        ghosted: a(23),
        rim: a(24),
        alert: a(25),
    }
}

/// Bit of the synthetic vendor type.
pub const VENDOR_BIT: u32 = 20;
/// Bit of the synthetic trainer type.
pub const TRAINER_BIT: u32 = 21;

/// A synthetic icon table: `Quest` (bit 1), `Waypoint` (bit 47), a vendor type (bit 20,
/// with off-screen images and a menu image) and a trainer type (bit 21, relative z 1).
pub fn icons() -> IconTable {
    let mut t = IconTable::default();
    let on = |n: u32, h: u32| NoteIcons {
        normal: Some(art_ref(n)),
        selected: Some(art_ref(n)),
        highlight: Some(art_ref(h)),
    };
    t.insert(
        1 << 1,
        IconEntry {
            onscreen: on(26, 27),
            ..IconEntry::default()
        },
    );
    t.insert(
        1 << 47,
        IconEntry {
            onscreen: on(28, 27),
            ..IconEntry::default()
        },
    );
    t.insert(
        1 << VENDOR_BIT,
        IconEntry {
            onscreen: on(29, 27),
            offscreen: Some(NoteIcons::single(art_ref(31))),
            menu: Some(art_ref(32)),
            relative_z: 0,
        },
    );
    t.insert(
        1 << TRAINER_BIT,
        IconEntry {
            onscreen: on(30, 27),
            relative_z: 1,
            ..IconEntry::default()
        },
    );
    t
}

/// A UI on a `w` × `h` screen with the test environment.
pub fn ui(w: i32, h: i32) -> Ui {
    Ui::new(Box::new(TestEnv::new()), Vocab::default(), (w, h))
}

/// A UI with layouts.
pub fn ui_layouts(w: i32, h: i32, layouts: Vec<UiLayout>) -> Ui {
    let mut env = TestEnv::new();
    for l in layouts {
        env.layouts.insert(l.did.0, Arc::new(l));
    }
    Ui::new(Box::new(env), Vocab::default(), (w, h))
}

/// The vocabularies (placeholders).
pub fn vocabs() -> (GameVocab, P5Vocab, NpcVocab) {
    (
        GameVocab::default(),
        P5Vocab::default(),
        NpcVocab::default(),
    )
}

/// A game-code message from `k`.
pub fn message(ui: &Ui, k: ElementKey, msg: u32, command: Option<&str>) -> UiEvent {
    UiEvent::Game {
        message: Message {
            seq: 0,
            source: k,
            source_id: ui.element_id(k).unwrap_or(0),
            msg,
            arg: MsgArg {
                command: command.map(Command::named),
                ..MsgArg::default()
            },
        },
    }
}

/// `Button_click` from `k`.
pub fn button(ui: &Ui, k: ElementKey) -> UiEvent {
    let m = ui.vocab().msgs.button_click;
    message(ui, k, m, None)
}

/// A left mouse click on `k`.
pub fn left(ui: &Ui, k: ElementKey) -> UiEvent {
    let m = ui.vocab().msgs.mouse_click;
    message(ui, k, m, Some("SelectLeft"))
}

/// A right mouse click on `k`.
pub fn right(ui: &Ui, k: ElementKey) -> UiEvent {
    let m = ui.vocab().msgs.mouse_click;
    message(ui, k, m, Some("SelectRight"))
}

/// A double-click on `k`.
pub fn double(ui: &Ui, k: ElementKey) -> UiEvent {
    let m = ui.vocab().msgs.mouse_dbl_click;
    message(ui, k, m, Some("SelectDblLeft"))
}

/// The first element with ID `id` at or below `k`.
pub fn part(ui: &Ui, k: ElementKey, id: u32) -> ElementKey {
    if ui.element_id(k) == Some(id) {
        return k;
    }
    tessera_ui::game::find_below(ui, k, id).unwrap_or_else(|| panic!("no element {id:#x}"))
}

/// A real left click at (`x`, `y`): pointer move, press and release of `SelectLeft`.
/// Returns the events the UI posted for it (earlier events are dropped first).
pub fn real_click_at(ui: &mut Ui, x: i32, y: i32) -> Vec<UiEvent> {
    let _ = ui.take_events();
    ui.pointer_moved(x, y);
    let c = Command::named("SelectLeft");
    ui.mouse_press(&c);
    ui.mouse_release(&c);
    ui.take_events()
}

/// A real left click at the centre of `k`'s displayed box (after settling the layout).
pub fn real_click(ui: &mut Ui, k: ElementKey) -> Vec<UiEvent> {
    settle(ui);
    let b = ui.displayed_box(k);
    real_click_at(ui, b.l + b.w() / 2, b.t + b.h() / 2)
}

/// Paints until settled.
pub fn settle(ui: &mut Ui) {
    let mut budget = Unlimited;
    for t in 0..6u64 {
        ui.paint(Duration::from_millis(16 * t + 1), &mut budget);
    }
}

/// An RGBA image of a painted window.
pub struct Rgba {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

/// Paints and reads the canvas of root panel `k` as RGBA.
pub fn capture(ui: &mut Ui, k: ElementKey) -> Rgba {
    settle(ui);
    let b = ui.displayed_box(k);
    let (w, h) = (b.w().max(1) as u32, b.h().max(1) as u32);
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let p = ui
                .surface_pixel(k, x, y)
                .expect("the window owns a surface");
            data.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, p as u8, (p >> 24) as u8]);
        }
    }
    Rgba {
        width: w,
        height: h,
        data,
    }
}

fn golden_path(stem: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join(format!("{stem}.png"))
}

fn write_png(path: &std::path::Path, img: &Rgba) {
    let f = std::fs::File::create(path).expect("create png");
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), img.width, img.height);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.set_compression(png::Compression::High);
    let mut w = e.write_header().expect("png header");
    w.write_image_data(&img.data).expect("png data");
}

fn read_png(path: &std::path::Path) -> Option<Rgba> {
    let f = std::fs::File::open(path).ok()?;
    let d = png::Decoder::new(std::io::BufReader::new(f));
    let mut r = d.read_info().ok()?;
    let mut buf = vec![0; r.output_buffer_size()?];
    let info = r.next_frame(&mut buf).ok()?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return None;
    }
    buf.truncate(info.buffer_size());
    Some(Rgba {
        width: info.width,
        height: info.height,
        data: buf,
    })
}

/// Decision 0031 tier 1: compares `img` with `tests/golden/p5_<name>.png` with the CPU
/// painter's tolerance (one step per channel, no neighbourhood search, no outliers);
/// `TESSERA_BLESS=1` rewrites the image and passes. On a mismatch the actual image goes
/// to the target's temporary directory.
pub fn golden(name: &str, img: &Rgba) {
    golden_file(&format!("p5_{name}"), img);
}

/// [`golden`] for `tests/golden/<stem>.png` (the goldens of other milestones that share
/// this support, such as `p4_<name>`).
pub fn golden_file(stem: &str, img: &Rgba) {
    let name = stem;
    let path = golden_path(stem);
    if std::env::var_os("TESSERA_BLESS").is_some_and(|v| v == "1") {
        std::fs::create_dir_all(path.parent().expect("dir")).expect("golden dir");
        write_png(&path, img);
        eprintln!("golden: blessed {}", path.display());
        return;
    }
    let Some(want) = read_png(&path) else {
        panic!(
            "golden {} missing or unreadable; run with TESSERA_BLESS=1 and check it by eye",
            path.display()
        );
    };
    let actual = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.actual.png"));
    if (want.width, want.height) != (img.width, img.height) {
        write_png(&actual, img);
        panic!(
            "golden {name}: size {}x{} != {}x{}; actual at {}",
            img.width,
            img.height,
            want.width,
            want.height,
            actual.display()
        );
    }
    let mut bad = 0usize;
    let mut first = None;
    for (i, (a, b)) in img.data.chunks(4).zip(want.data.chunks(4)).enumerate() {
        let d = a
            .iter()
            .zip(b)
            .map(|(x, y)| x.abs_diff(*y))
            .max()
            .unwrap_or(0);
        if d > 1 {
            bad += 1;
            first.get_or_insert((i as u32 % img.width, i as u32 / img.width, d));
        }
    }
    if bad > 0 {
        write_png(&actual, img);
        panic!(
            "golden {name}: {bad} pixels differ (first {first:?}); actual at {}",
            actual.display()
        );
    }
}

/// Test-side composition of nested surfaces over a captured canvas (the UI pass composes
/// them on the GPU): each element of `children` that owns a surface and is displayed is
/// drawn over `base` at its displayed box relative to `root`, turned about its centre by
/// its held rotation (nearest texel), with straight-alpha "over" blending. Enough for the
/// leaf surfaces of the P5 goldens (player markers, the minimap's rim notes).
pub fn overlay_surfaces(ui: &Ui, root: ElementKey, base: &mut Rgba, children: &[ElementKey]) {
    let rb = ui.displayed_box(root);
    for &k in children {
        if !ui.owns_surface(k) || !ui.is_displayed(k) {
            continue;
        }
        let Some(lb) = ui.layout_box(k) else { continue };
        let (cw, ch) = (lb.w(), lb.h());
        let d = ui.displayed_box(k);
        let (dw, dh) = (d.w().max(1), d.h().max(1));
        let cx = (d.l - rb.l) as f32 + dw as f32 / 2.0;
        let cy = (d.t - rb.t) as f32 + dh as f32 / 2.0;
        let angle = tessera_ui::game::held_rotation(ui, k)[2].to_radians();
        let (s, c) = angle.sin_cos();
        let reach = ((dw * dw + dh * dh) as f32).sqrt() / 2.0 + 1.0;
        let (x0, x1) = ((cx - reach).floor() as i32, (cx + reach).ceil() as i32);
        let (y0, y1) = ((cy - reach).floor() as i32, (cy + reach).ceil() as i32);
        for y in y0..y1 {
            for x in x0..x1 {
                if x < 0 || y < 0 || x >= base.width as i32 || y >= base.height as i32 {
                    continue;
                }
                // Screen y points down; a counter-clockwise turn on screen.
                let (vx, vy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                let ux = vx * c - vy * s;
                let uy = vx * s + vy * c;
                let sx = ((ux + dw as f32 / 2.0) * cw as f32 / dw as f32).floor() as i32;
                let sy = ((uy + dh as f32 / 2.0) * ch as f32 / dh as f32).floor() as i32;
                if sx < 0 || sy < 0 || sx >= cw || sy >= ch {
                    continue;
                }
                let Some(p) = ui.surface_pixel(k, sx, sy) else {
                    continue;
                };
                let a = (p >> 24) & 0xFF;
                if a == 0 {
                    continue;
                }
                let i = ((y as u32 * base.width + x as u32) * 4) as usize;
                let src = [(p >> 16) & 0xFF, (p >> 8) & 0xFF, p & 0xFF];
                for (j, sv) in src.iter().enumerate() {
                    let dv = u32::from(base.data[i + j]);
                    base.data[i + j] = ((sv * a + dv * (255 - a)) / 255) as u8;
                }
                let da = u32::from(base.data[i + 3]);
                base.data[i + 3] = (a + da * (255 - a) / 255) as u8;
            }
        }
    }
}
