// SPDX-License-Identifier: GPL-3.0-or-later
//! Committed synthetic goldens of the UI-scaling images `ui-scaling.md` §9.3 G1 and G3-G8
//! (decision 0031 tier 1), rendered **through the now-live `tessera-ui` layout and paint**
//! (decision 0040, §5-§6) at the §9.2 named configurations C1-C12.
//!
//! Each scene is built from synthetic content the test generates (solid-colour fills over
//! invented layout records, no game data, no install file) and opened as a native root panel
//! through [`Ui::open_panel`]. The configuration is applied with [`Ui::set_ui_screen`] (the UI
//! screen *F* and safe area *A* of the §9.2 row), the tree is laid out and painted by the CPU
//! painter, and the surface is read back at its **content scale** (its physical box in window
//! pixels, §3.6, §6.1). The pictures are compared pixel-for-pixel (one step per channel, no
//! outliers) against `tests/golden/uiscale_<g>_<config>.png`, the way the committed glyph
//! golden G2 (`ui_scaling_glyph_golden.rs`) compares its ladder. `TESSERA_BLESS=1` rewrites
//! the images.
//!
//! Scope. The goldens that a single surface (or a handful of fit-root surfaces composed 1 : 1
//! onto the window, §6.6) can capture on the CPU are committed here: G1 (a frame), G3 (a
//! Type A full-cover fit root), G4 (Type B side panels), G5 (anchored HUD panels), G6 (a panel
//! fit to screen) and G8 (a fit group of layered roots). **G7** (a frame "in the middle of a
//! scale change, old canvases composed scaled") is a transient of the renderer's composition
//! pass — the GPU up-scales the *stale* canvas of the previous content scale onto the new
//! physical box for the frames before the surface repaints (§6.5, §6.6). `surface_pixel`
//! returns the freshly repainted canvas, not that stale-scaled composite, so G7 needs the
//! headless renderer (`tessera-render`, not linked here) and is left for a later tier-2 task;
//! this file notes it in its summary rather than forcing a CPU stand-in.

mod ui_common;

use std::path::PathBuf;
use std::time::Duration;

use tessera_formats::did::Did;
use tessera_ui::paint::Unlimited;
use tessera_ui::scaling::{self, UiScreen};
use tessera_ui::{ElementKey, Ui, object_mode};
use ui_common::*;

// ----- the §9.2 named configurations --------------------------------------------------------

/// A §9.2 row: window `wp × hp`, user scale `s`, safe aspect (`None` = `"full"`), and the
/// resulting UI screen `wu × hu` the row records (asserted as a guard, so each golden stays a
/// true oracle of the configuration).
#[derive(Clone, Copy)]
struct Config {
    name: &'static str,
    wp: i32,
    hp: i32,
    s: f64,
    aspect: Option<f64>,
    wu: i32,
    hu: i32,
}

/// The `"21:9"` safe-aspect preset (decision 0040: 1440 × 2.4 = 3456, the C8 safe width of
/// §5.1 and `ui_scaling_live.rs`).
const A21: Option<f64> = Some(2.4);

const C1: Config = cfg("c1", 1280, 720, 1.0, A21, 1365, 768);
const C2: Config = cfg("c2", 1920, 1080, 1.0, A21, 1920, 1080);
const C3: Config = cfg("c3", 1920, 1080, 1.25, A21, 1536, 864);
const C5: Config = cfg("c5", 3840, 2160, 1.5, A21, 2560, 1440);
const C6: Config = cfg("c6", 3840, 2160, 2.0, A21, 1920, 1080);
const C7: Config = cfg("c7", 3440, 1440, 1.0, A21, 3440, 1440);
const C8: Config = cfg("c8", 5120, 1440, 1.0, A21, 5120, 1440);
const C9: Config = cfg("c9", 5120, 1440, 1.0, None, 5120, 1440);
const C10: Config = cfg("c10", 1600, 1200, 1.0, A21, 1600, 1200);

const fn cfg(
    name: &'static str,
    wp: i32,
    hp: i32,
    s: f64,
    aspect: Option<f64>,
    wu: i32,
    hu: i32,
) -> Config {
    Config {
        name,
        wp,
        hp,
        s,
        aspect,
        wu,
        hu,
    }
}

impl Config {
    fn screen(&self) -> UiScreen {
        let se = scaling::effective_scale(self.s, self.wp, self.hp);
        UiScreen::new(self.wp, self.hp, se).expect("non-zero window")
    }
}

/// Applies a configuration to a UI and guards the §9.2 `Wu × Hu` (so a wrong screen is caught
/// before the golden is blessed).
fn apply(ui: &mut Ui, c: &Config) {
    let f = c.screen();
    ui.set_ui_screen(f, scaling::safe_area(&f, c.aspect));
    assert_eq!(ui.screen_size(), (c.wu, c.hu), "{} screen F", c.name);
}

// ----- the synthetic palette ----------------------------------------------------------------

const IMG: u32 = 0x41E0_0000;

// Opaque ARGB fills, each a solid image registered under IMG + its index.
const BG: u32 = 0; // 0xFF10_1018 window body
const TITLE: u32 = 1; // 0xFF4A_3A22 title bar
const CORNER: u32 = 2; // 0xFFC0_8040 frame corner
const EDGE: u32 = 3; // 0xFF80_6030 frame edge
const BODY: u32 = 4; // 0xFF20_3040 frame centre
const PANEL_L: u32 = 5; // 0xFF2E_5A8A left panel
const PANEL_R: u32 = 6; // 0xFF8A_2A20 right panel
const CONTENT: u32 = 7; // 0xFF40_A060 content box
const HUD_A: u32 = 8; // 0xFFB0_7020 top-left HUD
const HUD_B: u32 = 9; // 0xFF30_90C0 top-right HUD
const HUD_C: u32 = 10; // 0xFFC04060 bottom-left HUD
const HUD_D: u32 = 11; // 0xFF60_B040 centre HUD
const LAYER_0: u32 = 12; // 0xFF203A5A lower fit layer
const LAYER_1: u32 = 13; // 0xA0E0_C040 upper fit layer (translucent)
const SPARSE: u32 = 14; // 0xFFA050C0 sparse fit root

const PALETTE: [u32; 15] = [
    0xFF10_1018,
    0xFF4A_3A22,
    0xFFC0_8040,
    0xFF80_6030,
    0xFF20_3040,
    0xFF2E_5A8A,
    0xFF8A_2A20,
    0xFF40_A060,
    0xFFB0_7020,
    0xFF30_90C0,
    0xFFC0_4060,
    0xFF60_B040,
    0xFF20_3A5A,
    0xA0E0_C040,
    0xFFA0_50C0,
];

/// A memory environment with the whole palette registered as 4 × 4 solid images.
fn base_env() -> tessera_ui::MemoryEnv {
    let mut env = env_with(vec![]);
    for (i, argb) in PALETTE.iter().enumerate() {
        add_image(&mut env, IMG + i as u32, solid(4, 4, *argb));
    }
    env
}

/// A filled element: a field at (x, y, w, h) with edges `e`, its box drawn with palette
/// colour `c` (`DrawMode` 2 = Replace, so the box is a flat opaque colour).
fn fill(
    id: u32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    e: [u32; 4],
    c: u32,
) -> tessera_formats::ui_layout::ElementDesc {
    media(edges(field(id, x, y, w, h), e), image_item(IMG + c, 2))
}

// ----- reading a surface at its content scale -----------------------------------------------

struct Img {
    w: u32,
    h: u32,
    data: Vec<u8>,
}

fn settle(ui: &mut Ui) {
    let mut budget = Unlimited;
    for t in 0..6u64 {
        ui.paint(Duration::from_millis(16 * t + 1), &mut budget);
    }
}

/// Paints and reads the canvas of root panel `k` as RGBA at its **content scale** (its physical
/// box in window pixels, §6.1): at `Se` = 1 this is the UI-unit box, at other scales the
/// larger/smaller physical box, composed 1 : 1 (§6.6). The surface's canvas is indexed from its
/// own top-left, so `surface_pixel(k, x, y)` reads physical pixel `(x, y)` of the canvas.
fn capture_phys(ui: &mut Ui, k: ElementKey) -> Img {
    settle(ui);
    let b = ui.physical_box(k);
    let (w, h) = (b.width().max(1), b.height().max(1));
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let p = ui
                .surface_pixel(k, x, y)
                .expect("the root panel owns a surface");
            data.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, p as u8, (p >> 24) as u8]);
        }
    }
    Img { w, h, data }
}

/// Composes several root surfaces onto the physical window `wp × hp` (§6.6: every surface drawn
/// 1 : 1 onto its physical box). The window starts transparent; each fit group's letterbox bars
/// (§5.4 step 6), as the UI reports them ([`Ui::letterbox_bars`]), are drawn as opaque black
/// fills directly below the group's lowest shown member, so a bar reads black only where the
/// layout says it is. Straight-alpha "over", so a translucent upper layer shows the lower one
/// through it.
fn compose_phys(ui: &mut Ui, roots: &[ElementKey], wp: i32, hp: i32) -> Img {
    settle(ui);
    let (w, h) = (wp.max(1) as u32, hp.max(1) as u32);
    let mut data = vec![0u8; (w * h * 4) as usize];
    let bars = ui.letterbox_bars();
    for &k in roots {
        for g in bars.iter().filter(|g| g.below == k) {
            for r in &g.rects {
                for y in r.y0.max(0)..r.y1.min(hp) {
                    for x in r.x0.max(0)..r.x1.min(wp) {
                        let i = ((y as u32 * w + x as u32) * 4) as usize;
                        data[i..i + 4].copy_from_slice(&[0, 0, 0, 255]);
                    }
                }
            }
        }
        let b = ui.physical_box(k);
        for cy in 0..b.height() as i32 {
            for cx in 0..b.width() as i32 {
                let (wx, wy) = (b.x0 + cx, b.y0 + cy);
                if wx < 0 || wy < 0 || wx >= wp || wy >= hp {
                    continue;
                }
                let Some(px) = ui.surface_pixel(k, cx, cy) else {
                    continue;
                };
                let a = (px >> 24) & 0xFF;
                if a == 0 {
                    continue;
                }
                let i = ((wy as u32 * w + wx as u32) * 4) as usize;
                let src = [(px >> 16) & 0xFF, (px >> 8) & 0xFF, px & 0xFF];
                for (j, sv) in src.iter().enumerate() {
                    let dv = u32::from(data[i + j]);
                    data[i + j] = ((sv * a + dv * (255 - a)) / 255) as u8;
                }
                let da = u32::from(data[i + 3]);
                data[i + 3] = (a + da * (255 - a) / 255) as u8;
            }
        }
    }
    Img { w, h, data }
}

/// Asserts the image holds at least one pixel of the opaque palette colour `c` (a cheap guard
/// that the region actually painted, so a blank capture is never blessed).
fn assert_has(img: &Img, c: u32, what: &str) {
    let want = [(c >> 16) as u8, (c >> 8) as u8, c as u8];
    let found = img.data.chunks(4).any(|p| {
        p[3] == 0xFF
            && p[0].abs_diff(want[0]) <= 1
            && p[1].abs_diff(want[1]) <= 1
            && p[2].abs_diff(want[2]) <= 1
    });
    assert!(found, "no {what} pixels (colour {c:#010x}) in the capture");
}

// ----- golden i/o (as `ui_scaling_glyph_golden.rs`) -----------------------------------------

fn golden_path(stem: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join(format!("{stem}.png"))
}

fn write_png(path: &std::path::Path, img: &Img) {
    let f = std::fs::File::create(path).expect("create png");
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), img.w, img.h);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.set_compression(png::Compression::High);
    let mut w = e.write_header().expect("png header");
    w.write_image_data(&img.data).expect("png data");
}

fn read_png(path: &std::path::Path) -> Option<Img> {
    let f = std::fs::File::open(path).ok()?;
    let d = png::Decoder::new(std::io::BufReader::new(f));
    let mut r = d.read_info().ok()?;
    let mut buf = vec![0; r.output_buffer_size()?];
    let info = r.next_frame(&mut buf).ok()?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return None;
    }
    buf.truncate(info.buffer_size());
    Some(Img {
        w: info.width,
        h: info.height,
        data: buf,
    })
}

/// Decision 0031 tier 1: compares `img` with `tests/golden/<stem>.png` (one step per channel,
/// no outliers). `TESSERA_BLESS=1` rewrites it.
fn golden(stem: &str, img: &Img) {
    let path = golden_path(stem);
    if std::env::var_os("TESSERA_BLESS").is_some_and(|v| v == "1") {
        std::fs::create_dir_all(path.parent().expect("dir")).expect("golden dir");
        write_png(&path, img);
        eprintln!("golden: blessed {}", path.display());
        return;
    }
    let Some(want) = read_png(&path) else {
        panic!(
            "golden {} missing; run with TESSERA_BLESS=1 and check it by eye",
            path.display()
        );
    };
    let actual = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{stem}.actual.png"));
    if (want.w, want.h) != (img.w, img.h) {
        write_png(&actual, img);
        panic!(
            "golden {stem}: size {}x{} != {}x{}; actual at {}",
            img.w,
            img.h,
            want.w,
            want.h,
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
            first.get_or_insert((i as u32 % img.w, i as u32 / img.w, d));
        }
    }
    if bad > 0 {
        write_png(&actual, img);
        panic!(
            "golden {stem}: {bad} pixels differ (first {first:?}); actual at {}",
            actual.display()
        );
    }
}

// ============================================================================================
// G1 frame: a nine-element frame with 16 × 16 corners, edges and a fitted title bar.
// Configurations C1, C2, C3, C5, C6.
// ============================================================================================

const G1_LAYOUT: u32 = 0x22E0_0100;
const G1_ROOT: u32 = 0x1000_0101;

/// A 420 × 300 window panel (a safe-area panel root, KEEP edges) centred in a 1024 × 768 design
/// area, with a nine-slice frame: a body fill, four 16 × 16 corners pinned to each corner, four
/// edges stretched between them, and a title bar across the top spanning the inner width (its
/// horizontal edges SAME, so it follows the panel), all scaled with the panel's content scale.
/// A fit-container title bar would own its own surface (object mode Stretch); this title stays
/// in the panel's surface so the whole frame is one capture.
fn build_g1() -> Ui {
    let (pw, ph) = (420, 300);
    let c = 16; // corner size
    // The nine frame slices and the title bar, all direct children of the panel (so they share
    // the panel's one surface). Drawn in order: body, four edges, title, four corners on top.
    let pieces = [
        fill(
            0x1000_0111,
            c,
            c,
            pw - 2 * c,
            ph - 2 * c,
            [0, 0, 0, 0],
            BODY,
        ),
        fill(0x1000_0112, c, 0, pw - 2 * c, c, [0, 0, 0, 0], EDGE), // top
        fill(0x1000_0113, c, ph - c, pw - 2 * c, c, [0, 0, 0, 0], EDGE), // bottom
        fill(0x1000_0114, 0, c, c, ph - 2 * c, [0, 0, 0, 0], EDGE), // left
        fill(0x1000_0115, pw - c, c, c, ph - 2 * c, [0, 0, 0, 0], EDGE), // right
        fill(0x1000_0110, c, 4, pw - 2 * c, 24, [5, 5, 5, 0], TITLE), // title bar
        fill(0x1000_0116, 0, 0, c, c, [0, 0, 0, 0], CORNER),
        fill(0x1000_0117, pw - c, 0, c, c, [0, 0, 0, 0], CORNER),
        fill(0x1000_0118, 0, ph - c, c, c, [0, 0, 0, 0], CORNER),
        fill(0x1000_0119, pw - c, ph - c, c, c, [0, 0, 0, 0], CORNER),
    ];
    let mut root = fill(
        G1_ROOT,
        (1024 - pw) / 2,
        (768 - ph) / 2,
        pw,
        ph,
        [0, 0, 0, 0],
        BG,
    );
    for p in pieces {
        root = child(root, p);
    }
    let mut env = base_env();
    env.add_layout(layout(G1_LAYOUT, 1024, 768, vec![root]));
    ui_with(env, 1024, 768)
}

fn g1(c: &Config) {
    let mut ui = build_g1();
    let r = ui.open_panel(Did(G1_LAYOUT), G1_ROOT).unwrap();
    apply(&mut ui, c);
    let img = capture_phys(&mut ui, r);
    assert_has(&img, CORNER_C, "frame corner");
    assert_has(&img, TITLE_C, "title bar");
    assert_has(&img, BODY_C, "frame body");
    golden(&format!("uiscale_g1_frame_{}", c.name), &img);
}

const CORNER_C: u32 = PALETTE[CORNER as usize];
const TITLE_C: u32 = PALETTE[TITLE as usize];
const BODY_C: u32 = PALETTE[BODY as usize];

#[test]
fn g1_frame_c1() {
    g1(&C1);
}
#[test]
fn g1_frame_c2() {
    g1(&C2);
}
#[test]
fn g1_frame_c3() {
    g1(&C3);
}
#[test]
fn g1_frame_c5() {
    g1(&C5);
}
#[test]
fn g1_frame_c6() {
    g1(&C6);
}

// ============================================================================================
// G3 fit A: a 2560 × 1080 full-cover fit container with a centred-bottom content box.
// Configurations C1, C2, C7, C8, C9, C10.
// ============================================================================================

const G3_LAYOUT: u32 = 0x22E0_0300;
const G3_ROOT: u32 = 0x1000_0301;

/// A 2560 × 1080 Type A fit root (both horizontal edges 6, covers its design area), a dark body
/// fill and one child content box 900 × 360 centred along the bottom. The content box is the
/// Type A content `C`, so the magnification bound and the centring drive the layout.
fn build_g3() -> Ui {
    let (dw, dh) = (2560, 1080);
    let (cw, ch) = (900, 360);
    let content = fill(
        0x1000_0302,
        (dw - cw) / 2,
        dh - ch - 40,
        cw,
        ch,
        [0, 0, 0, 0],
        CONTENT,
    );
    let root = child(fill(G3_ROOT, 0, 0, dw, dh, [6, 5, 6, 5], BG), content);
    let mut env = base_env();
    env.add_layout(layout(G3_LAYOUT, dw as u32, dh as u32, vec![root]));
    ui_with(env, dw, dh)
}

fn g3(c: &Config) {
    let mut ui = build_g3();
    let r = ui.open_panel(Did(G3_LAYOUT), G3_ROOT).unwrap();
    apply(&mut ui, c);
    let img = capture_phys(&mut ui, r);
    assert_has(&img, PALETTE[CONTENT as usize], "content box");
    assert_has(&img, PALETTE[BG as usize], "fit-root body");
    golden(&format!("uiscale_g3_fita_{}", c.name), &img);
}

#[test]
fn g3_fita_c1() {
    g3(&C1);
}
#[test]
fn g3_fita_c2() {
    g3(&C2);
}
#[test]
fn g3_fita_c7() {
    g3(&C7);
}
#[test]
fn g3_fita_c8() {
    g3(&C8);
}
#[test]
fn g3_fita_c9() {
    g3(&C9);
}
#[test]
fn g3_fita_c10() {
    g3(&C10);
}

// ============================================================================================
// G4 fit B: a 1440 × 900 root with left and right side panels (Type B fit containers).
// Configurations C1, C7, C8, C10.
// ============================================================================================

const G4_LAYOUT: u32 = 0x22E0_0400;
const G4_ROOT: u32 = 0x1000_0401;

/// A 1440 × 900 full-cover fit root with a dark body and two side panels that are Type B fit
/// containers (object mode Stretch, exactly one horizontal edge 6): the left panel's right edge
/// is 6 (anchored at the computed left, height-driven), the right panel's left edge is 6.
fn build_g4() -> Ui {
    let (dw, dh) = (1440, 900);
    let left = {
        let p = fill(0x1000_0402, 0, 0, 260, dh, [5, 5, 6, 5], PANEL_L);
        prop(p, v().pids.object_mode, en(object_mode::STRETCH))
    };
    let right = {
        let p = fill(0x1000_0403, dw - 260, 0, 260, dh, [6, 5, 5, 5], PANEL_R);
        prop(p, v().pids.object_mode, en(object_mode::STRETCH))
    };
    let root = child(
        child(fill(G4_ROOT, 0, 0, dw, dh, [6, 5, 6, 5], BG), left),
        right,
    );
    let mut env = base_env();
    env.add_layout(layout(G4_LAYOUT, dw as u32, dh as u32, vec![root]));
    ui_with(env, dw, dh)
}

fn g4(c: &Config) {
    let mut ui = build_g4();
    let r = ui.open_panel(Did(G4_LAYOUT), G4_ROOT).unwrap();
    // The side panels are Type B fit containers (object mode Stretch), so each owns its own
    // surface; compose them with the root onto the physical window (§6.6).
    let left = ui.find_child(r, 0x1000_0402).unwrap();
    let right = ui.find_child(r, 0x1000_0403).unwrap();
    apply(&mut ui, c);
    let img = compose_phys(&mut ui, &[r, left, right], c.wp, c.hp);
    assert_has(&img, PALETTE[PANEL_L as usize], "left panel");
    assert_has(&img, PALETTE[PANEL_R as usize], "right panel");
    golden(&format!("uiscale_g4_fitb_{}", c.name), &img);
}

#[test]
fn g4_fitb_c1() {
    g4(&C1);
}
#[test]
fn g4_fitb_c7() {
    g4(&C7);
}
#[test]
fn g4_fitb_c8() {
    g4(&C8);
}
#[test]
fn g4_fitb_c10() {
    g4(&C10);
}

// ============================================================================================
// G5 HUD: four anchored panels (edge modes 4/5, 1 and 0). Configurations C2, C8, C9.
//
// The four panels are children of one full-screen (5, 5, 5, 5) root that owns the surface, so
// the capture is their composite; each is anchored to a different corner by its edges, so the
// ultrawide configurations (C8, C9) show them pinned to the safe-area / screen edges.
// ============================================================================================

const G5_LAYOUT: u32 = 0x22E0_0500;
const G5_ROOT: u32 = 0x1000_0501;

fn build_g5() -> Ui {
    let (dw, dh) = (1024, 768);
    // Top-left: both near edges SAME (anchored to top-left).
    let tl = fill(0x1000_0502, 16, 16, 240, 90, [5, 5, 5, 5], HUD_A);
    // Top-right: left OPPOSITE (4), right SAME (5) — pinned to the right.
    let tr = fill(0x1000_0503, dw - 256, 16, 240, 90, [4, 5, 5, 0], HUD_B);
    // Bottom-left: top OPPOSITE (4), bottom KEEP (1) — pinned to the bottom-left.
    let bl = fill(0x1000_0504, 16, dh - 106, 240, 90, [5, 4, 0, 1], HUD_C);
    // Centre: both horizontal edges OPPOSITE+SAME around the centre line (KEEP, mode 0) so it
    // stays mid-width.
    let mid = fill(
        0x1000_0505,
        dw / 2 - 160,
        dh - 70,
        320,
        54,
        [0, 4, 0, 1],
        HUD_D,
    );
    let mut root = fill(G5_ROOT, 0, 0, dw, dh, [5, 5, 5, 5], BG);
    for p in [tl, tr, bl, mid] {
        root = child(root, p);
    }
    let mut env = base_env();
    env.add_layout(layout(G5_LAYOUT, dw as u32, dh as u32, vec![root]));
    ui_with(env, dw, dh)
}

fn g5(c: &Config) {
    let mut ui = build_g5();
    let r = ui.open_panel(Did(G5_LAYOUT), G5_ROOT).unwrap();
    apply(&mut ui, c);
    let img = capture_phys(&mut ui, r);
    for (col, what) in [
        (HUD_A, "top-left"),
        (HUD_B, "top-right"),
        (HUD_C, "bottom-left"),
        (HUD_D, "centre"),
    ] {
        assert_has(&img, PALETTE[col as usize], what);
    }
    golden(&format!("uiscale_g5_hud_{}", c.name), &img);
}

#[test]
fn g5_hud_c2() {
    g5(&C2);
}
#[test]
fn g5_hud_c8() {
    g5(&C8);
}
#[test]
fn g5_hud_c9() {
    g5(&C9);
}

// ============================================================================================
// G6 too large: an 1100 × 800 panel fit to screen at C1 (F = 1365 × 768, height 768 < 800, so
// §4.3 / §5.6 shrink it with a uniform node scale). Configuration C1.
// ============================================================================================

const G6_LAYOUT: u32 = 0x22E0_0600;
const G6_ROOT: u32 = 0x1000_0601;

fn build_g6() -> Ui {
    let (pw, ph) = (1100, 800);
    // A safe-area panel (KEEP edges, does not cover the 1200 × 1000 design area) with a bright
    // inner box so the shrink is visible against the body.
    let inner = fill(
        0x1000_0602,
        60,
        60,
        pw - 120,
        ph - 120,
        [0, 0, 0, 0],
        CONTENT,
    );
    let root = child(fill(G6_ROOT, 10, 10, pw, ph, [0, 0, 0, 0], BG), inner);
    let mut env = base_env();
    env.add_layout(layout(G6_LAYOUT, 1200, 1000, vec![root]));
    ui_with(env, 1200, 1000)
}

#[test]
fn g6_toolarge_c1() {
    let mut ui = build_g6();
    let r = ui.open_panel(Did(G6_LAYOUT), G6_ROOT).unwrap();
    apply(&mut ui, &C1);
    // The panel must have been shrunk to fit the 768-tall screen: its physical height ≤ 768.
    let ph = ui.physical_box(r).height();
    assert!(ph <= 768, "G6: panel physical height {ph} fits the screen");
    let img = capture_phys(&mut ui, r);
    assert_has(&img, PALETTE[CONTENT as usize], "inner box");
    golden("uiscale_g6_toolarge_c1", &img);
}

// ============================================================================================
// G8 fit group: two layered 1024 × 768 fit roots (§10.6) plus a sparse 1440 × 900 fit root
// (§10.7), composed onto the window. Configurations C1, C2, C7, C8, C9.
//
// The lower layer's content covers its whole area (like `StoryField`); the upper layer carries
// a small translucent content box near the bottom. Both are full-cover Type A fit roots of one
// layout, so they form **one fit group** (§5.4): one factor, one placement, the factor the
// §10.6 table gives (1.4063 at C2, not the cover factor 1.875 the upper layer would get on its
// own). The sparse root is the only fit root of its layout (a group of one) whose single child
// is the §10.7 tutorial-chooser content (735 × 231 at (224, 0)), so the magnification bound
// stops it at 2.1333 on the 1440-row screens. Each group's letterbox bars are drawn below its
// lowest shown member.
// ============================================================================================

const G8_LAYOUT: u32 = 0x22E0_0800;
const G8_LOWER: u32 = 0x1000_0801;
const G8_UPPER: u32 = 0x1000_0802;
const G8_SPARSE: u32 = 0x1000_0803;

fn build_g8() -> Ui {
    // Lower fit root: its content covers the whole 1024 × 768 area.
    let lower = child(
        edges(field(G8_LOWER, 0, 0, 1024, 768), [6, 5, 6, 5]),
        fill(0x1000_0806, 0, 0, 1024, 768, [0, 0, 0, 0], LAYER_0),
    );
    // Upper fit root: a full-cover area that is otherwise transparent, with one small
    // translucent content box near the bottom (so the lower layer shows through around it).
    let upper = child(
        edges(field(G8_UPPER, 0, 0, 1024, 768), [6, 5, 6, 5]),
        fill(0x1000_0804, 312, 560, 400, 150, [0, 0, 0, 0], LAYER_1),
    );
    // Sparse fit root: a full-cover 1440 × 900 area with the §10.7 content box.
    let sparse = child(
        edges(field(G8_SPARSE, 0, 0, 1440, 900), [6, 5, 6, 5]),
        fill(0x1000_0805, 224, 0, 735, 231, [0, 0, 0, 0], SPARSE),
    );
    let mut env = base_env();
    env.add_layout(layout(G8_LAYOUT, 1024, 768, vec![lower, upper]));
    env.add_layout(layout(G8_LAYOUT + 1, 1440, 900, vec![sparse]));
    ui_with(env, 1024, 768)
}

/// The fit factors (§10.6 theatre group, §10.7 sparse root) a G8 configuration must show.
fn g8_factors(c: &Config) -> (f32, f32) {
    match c.name {
        "c1" => (1.0, 0.9479),
        "c2" => (1.4063, 1.3333),
        _ => (1.875, 2.1333), // C7, C8, C9
    }
}

fn g8(c: &Config) {
    let mut ui = build_g8();
    let lo = ui.open_panel(Did(G8_LAYOUT), G8_LOWER).unwrap();
    let up = ui.open_panel(Did(G8_LAYOUT), G8_UPPER).unwrap();
    let sp = ui.open_panel(Did(G8_LAYOUT + 1), G8_SPARSE).unwrap();
    apply(&mut ui, c);
    // One group: one factor and one displayed box for both layers (I16), the §10.6 value.
    let (fa, fs) = g8_factors(c);
    assert_eq!(
        ui.fit_factor(lo),
        ui.fit_factor(up),
        "{}: one group factor",
        c.name
    );
    assert_eq!(
        ui.displayed_box(lo),
        ui.displayed_box(up),
        "{}: one box",
        c.name
    );
    assert!(
        (ui.fit_factor(lo) - fa).abs() < 1e-3,
        "{}: group f {} vs {fa}",
        c.name,
        ui.fit_factor(lo)
    );
    assert!(
        (ui.fit_factor(sp) - fs).abs() < 1e-3,
        "{}: sparse f {} vs {fs}",
        c.name,
        ui.fit_factor(sp)
    );
    let img = compose_phys(&mut ui, &[lo, up, sp], c.wp, c.hp);
    assert_has(&img, PALETTE[SPARSE as usize], "sparse content");
    assert_has(&img, PALETTE[LAYER_0 as usize], "lower layer");
    golden(&format!("uiscale_g8_fitgroup_{}", c.name), &img);
}

#[test]
fn g8_fitgroup_c1() {
    g8(&C1);
}
#[test]
fn g8_fitgroup_c2() {
    g8(&C2);
}
#[test]
fn g8_fitgroup_c7() {
    g8(&C7);
}
#[test]
fn g8_fitgroup_c8() {
    g8(&C8);
}
#[test]
fn g8_fitgroup_c9() {
    g8(&C9);
}
