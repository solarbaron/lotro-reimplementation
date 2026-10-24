// SPDX-License-Identifier: GPL-3.0-or-later
//! Test support of the P3 window tests: an environment that shapes every text with
//! Tessera's bundled font (`fonts.md` §11) and serves generated art, click events, painting
//! a window into an RGBA image, and the committed-golden check of decision 0031 tier 1.
//! All values are invented; no install file is opened.
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tessera_formats::did::Did;
use tessera_formats::property::PropertyValue;
use tessera_formats::ui_layout::UiLayout;
use tessera_material::ui_image::{PixelLayout, UiImage};
use tessera_ui::events::{Message, MsgArg};
use tessera_ui::game::NpcVocab;
use tessera_ui::game::npc::FallbackArt;
use tessera_ui::paint::Unlimited;
use tessera_ui::text::{
    CharListBuilder, DEFAULT_RANGES, FontFace, LayoutParams, TesseraFont, layout,
};
use tessera_ui::{
    Command, ElementKey, ImageRef, ImageState, PreparedText, TextRequest, Ui, UiEnv, UiEvent, Vocab,
};

/// First DID of the generated art.
pub const ART: u32 = 0x41FF_0000;

/// The environment: generated images by DID, text shaped with the bundled font at 12 px.
pub struct TestEnv {
    pub images: HashMap<ImageRef, Arc<UiImage>>,
    face: Arc<FontFace>,
    colour_pid: u32,
}

impl TestEnv {
    pub fn new() -> Self {
        let font = TesseraFont::bundled().expect("bundled font");
        let face = Arc::new(font.rasterise(12, DEFAULT_RANGES).expect("rasterise"));
        let mut env = Self {
            images: HashMap::new(),
            face,
            colour_pid: NpcVocab::default().pids.text_font_color,
        };
        for (i, img) in generated_art().into_iter().enumerate() {
            env.images
                .insert(ImageRef::Did(ART + i as u32), Arc::new(img));
        }
        env
    }
}

impl UiEnv for TestEnv {
    fn layout(&mut self, _did: Did) -> Option<Arc<UiLayout>> {
        None
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
        let p = LayoutParams {
            width,
            height: r.size.1.max(0),
            ..LayoutParams::default()
        };
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

fn solid(w: u32, h: u32, argb: u32) -> UiImage {
    img(w, h, vec![argb; (w * h) as usize])
}

/// A vertical gradient 1 pixel wide (tiles horizontally).
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

/// A disc of `d` pixels in `argb` on transparent.
fn disc(d: u32, argb: u32) -> UiImage {
    let r = d as i32;
    let px = (0..d * d)
        .map(|i| {
            let (x, y) = ((i % d) as i32 * 2 + 1 - r, (i / d) as i32 * 2 + 1 - r);
            if x * x + y * y <= r * r { argb } else { 0 }
        })
        .collect();
    img(d, d, px)
}

/// A framed rectangle of `w` × `h`: border `edge`, fill `fill`.
fn rect(w: u32, h: u32, edge: u32, fill: u32) -> UiImage {
    let px = (0..w * h)
        .map(|i| {
            let (x, y) = (i % w, i / w);
            if x == 0 || y == 0 || x == w - 1 || y == h - 1 {
                edge
            } else {
                fill
            }
        })
        .collect();
    img(w, h, px)
}

/// A framed square icon of `d` pixels: border `edge`, fill `fill`, a diagonal in `edge`.
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

/// The generated art, by index from [`ART`].
fn generated_art() -> Vec<UiImage> {
    vec![
        solid(8, 8, 0xF01A_1C24),               // 0 window
        vgrad(24, 0xFF4A_3A22, 0xFF22_1A10),    // 1 title bar
        solid(8, 8, 0xC00C_0E14),               // 2 panel
        vgrad(64, 0xFF26_2A36, 0xFF1A_1D26),    // 3 row
        vgrad(64, 0xFF46_4A26, 0xFF2E_3018),    // 4 row highlight
        vgrad(24, 0xFF5A_4A30, 0xFF3A_2E1C),    // 5 button
        vgrad(24, 0xFF40_4040, 0xFF30_3030),    // 6 button ghosted
        solid(16, 16, 0xFF8A_2A20),             // 7 close
        vgrad(23, 0xFF30_3040, 0xFF20_2028),    // 8 tab
        vgrad(23, 0xFF60_5030, 0xFF40_3420),    // 9 tab selected
        solid(4, 4, 0xFF08_0808),               // 10 entry
        icon(32, 0xFF70_6050, 0xFF10_1010),     // 11 slot
        disc(14, 0xFFE8_C040),                  // 12 gold
        disc(14, 0xFFC8_C8D0),                  // 13 silver
        disc(14, 0xFFB0_6A30),                  // 14 copper
        disc(16, 0xFF40_E040),                  // 15 ready
        icon(20, 0xFF80_8080, 0xFF30_5070),     // 16 solo
        icon(20, 0xFF80_8080, 0xFF30_7050),     // 17 small fellowship
        icon(20, 0xFF80_8080, 0xFF70_5030),     // 18 fellowship
        icon(20, 0xFF80_8080, 0xFF70_3030),     // 19 raid
        icon(20, 0xFF80_8080, 0xFF60_6020),     // 20 repeatable
        icon(32, 0xFFC0_A060, 0xFF40_6090),     // 21 item icon A
        icon(32, 0xFF60_C0A0, 0xFF90_4040),     // 22 item icon B
        icon(32, 0xFFA0_A0FF, 0xFF30_3060),     // 23 item icon C
        disc(16, 0xFFFF_E000),                  // 24 quest note
        disc(16, 0xFFFF_FFFF),                  // 25 quest note highlight
        rect(26, 24, 0xFF90_9090, 0xFF50_3060), // 26 action icon
        rect(20, 20, 0xFFFF_E060, 0xFF60_5010), // 27 buy-mode overlay
    ]
}

/// The art reference of index `i`.
pub fn art_ref(i: u32) -> ImageRef {
    ImageRef::Did(ART + i)
}

/// The fallback art over the generated images.
pub fn art() -> FallbackArt {
    let a = |i| Some(art_ref(i));
    FallbackArt {
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
        ready: a(15),
        solo: a(16),
        small_fellowship: a(17),
        fellowship: a(18),
        raid: a(19),
        repeatable: a(20),
        mode_overlay: a(27),
    }
}

/// Item icons A, B, C.
pub fn item_icon(i: u32) -> tessera_ui::game::IconImage {
    tessera_ui::game::IconImage::Composed(art_ref(21 + i.min(2)))
}

/// A UI on a `w` × `h` screen with the test environment.
pub fn ui(w: i32, h: i32) -> Ui {
    Ui::new(Box::new(TestEnv::new()), Vocab::default(), (w, h))
}

/// The P3 vocabulary (placeholders).
pub fn nv() -> NpcVocab {
    NpcVocab::default()
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

/// Paints the frames [`settle`] paints without asserting that painting settled: for scenes
/// whose environment serves layouts only (the real-data tests), where surfaces that draw images
/// keep waiting for them and nothing is captured.
pub fn paint_frames(ui: &mut Ui) {
    let mut budget = Unlimited;
    for t in 0..6u64 {
        ui.paint(Duration::from_millis(16 * t + 1), &mut budget);
    }
    let _ = ui.capture_settle(Duration::from_millis(16 * 5 + 1), &mut budget, 256, |_| {});
}

/// Paints until settled, for a capture: asserts that the capture settle of `ui-scaling.md`
/// §9.3 finished, so that a golden never records a preview or an unfinished paint (item 4).
pub fn settle(ui: &mut Ui) {
    let mut budget = Unlimited;
    for t in 0..6u64 {
        ui.paint(Duration::from_millis(16 * t + 1), &mut budget);
    }
    // `ui-scaling.md` §9.3: a capture shows the settled picture, every geometry preview
    // settled on the held paint clock, with nothing else moved.
    assert!(
        ui.capture_settle(Duration::from_millis(16 * 5 + 1), &mut budget, 256, |_| {}),
        "the capture settle did not finish: a golden would record a preview (§9.3 item 4)"
    );
    assert!(
        !ui.geometry_preview_pending(),
        "no surface in a geometry preview"
    );
}

/// An RGBA image of a painted window.
pub struct Rgba {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

/// Paints and reads the canvas of root panel `k` as RGBA (canvas pixels are
/// `0xAARRGGBB`).
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

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join(format!("p3_{name}.png"))
}

fn write_png(path: &std::path::Path, img: &Rgba) {
    let f = std::fs::File::create(path).expect("create png");
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), img.width, img.height);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
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

/// Decision 0031 tier 1: compares `img` with `tests/golden/p3_<name>.png` with the CPU
/// painter's tolerance (one step per channel, no neighbourhood search, no outliers);
/// `TESSERA_BLESS=1` rewrites the image and passes. On a mismatch the actual image goes
/// to the target's temporary directory.
pub fn golden(name: &str, img: &Rgba) {
    let path = golden_path(name);
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
    let actual = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("p3_{name}.actual.png"));
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
