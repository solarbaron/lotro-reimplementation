// SPDX-License-Identifier: GPL-3.0-or-later
//! Committed synthetic golden G2 of `ui-scaling.md` §9.3 (decision 0031 tier 1): **the same
//! paragraph laid out and painted** at the content scales *c* = 1, 1.25, 1.5, 2 and 3 through
//! the live `tessera-ui` text layout and painter (§6.4: target heights, the font ladder,
//! derived glyphs, physical layout, §6.5 glyph cache).
//!
//! The font is a **synthetic bitmap ladder** in the form of `fonts.md` §3: five font records
//! the test generates from Tessera's bundled font (decision 0022) at the pixel sizes 10, 12,
//! 14, 16 and 20, registered as one ladder ([`Ui::set_font_ladders`]). The paragraph uses the
//! smallest record; at the five scales its target height is served by the exact record, by
//! downsampling a taller record and by the distance method from the tallest one (§6.4.3),
//! which the test asserts so the golden stays a true oracle. Each scale is a UI screen with
//! `Se` = *c* (a 1024 × 768 UI screen on a window of 1024 *c* × 768 *c*); the window control
//! is read back at its physical size and compared pixel-for-pixel (one step per channel, no
//! outliers) against `tests/golden/uiscale_g2_c<100·c>.png`. `TESSERA_BLESS=1` rewrites them.
//! No install file is opened.
//!
//! The *c* = 1 picture is also compared with the same UI without a UI screen (the identity,
//! invariant I2): byte for byte.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tessera_ui::paint::Unlimited;
use tessera_ui::scaling::{self, SourceKind, UiScreen};
use tessera_ui::text::{
    CharListBuilder, DEFAULT_RANGES, FontFace, FontId, FontLadder, LayoutParams, Margins,
    TesseraFont, layout,
};
use tessera_ui::{ControlKind, ElementKey, MemoryEnv, PreparedText, Ui};

/// The pixel sizes of the synthetic ladder's records (§9.3).
const LADDER_PX: [u32; 5] = [10, 12, 14, 16, 20];

/// The content scales of G2 (§9.3).
const SCALES: [f64; 5] = [1.0, 1.25, 1.5, 2.0, 3.0];

const PARAGRAPH: &str = "The quick brown fox jumps over the lazy dog. \
    Sphinx of black quartz, judge my vow! 0123456789 (a, b; c?)";

/// The label's layout box in UI units and its margins.
const LABEL: (i32, i32) = (300, 96);
const MARGINS: Margins = Margins {
    left: 4,
    right: 4,
    top: 2,
    bottom: 2,
};

/// The synthetic ladder: the bundled font rasterised at each size, re-labelled as data font
/// records (DIDs invented, prefix 0x42), so the ladder rules of §6.4.3 apply to them.
fn ladder() -> Vec<Arc<FontFace>> {
    let font = TesseraFont::bundled().expect("bundled font");
    LADDER_PX
        .iter()
        .map(|&px| {
            let f = font.rasterise(px, DEFAULT_RANGES).expect("rasterise");
            Arc::new(
                FontFace::from_parts(
                    FontId::Data(0x4200_0A00 + px),
                    f.line_height(),
                    f.baseline(),
                    f.borders(),
                    f.glyphs().to_vec(),
                    Arc::clone(f.foreground()),
                    f.background().cloned(),
                )
                .expect("face"),
            )
        })
        .collect()
}

fn params(size: (i32, i32), wrap: Option<i32>) -> LayoutParams {
    LayoutParams {
        width: wrap.unwrap_or(size.0),
        height: size.1,
        margins: MARGINS,
        ..LayoutParams::default()
    }
}

struct Scene {
    ui: Ui,
    window: ElementKey,
}

/// A window with a dark back colour holding one label with the paragraph, its text shaped at
/// layout units with the ladder's smallest record (as the client's shaper would).
fn scene(faces: &[Arc<FontFace>]) -> Scene {
    scene_with(faces, true)
}

/// [`scene`], with or without the layout-parameter reader (without one the scaled path keeps
/// the shaper's line breaks, `relayout_lines`: the path a client without a reader takes).
fn scene_with(faces: &[Arc<FontFace>], reader: bool) -> Scene {
    let base = Arc::clone(&faces[0]);
    let fonts = vec![Some(base)];
    let mut b = CharListBuilder::new();
    b.colour(0xFFF0_E8D0).push_str(PARAGRAPH);
    let chars = b.finish();
    let p = params(LABEL, None);
    let l = layout(&chars, &fonts, &p, None).expect("layout");
    let extent = (l.content_width, l.content_height);
    let mut env = MemoryEnv::default();
    env.texts.insert(
        PARAGRAPH.to_string(),
        Arc::new(PreparedText {
            layout: l,
            chars,
            fonts,
            outline: true,
            outline_colour: 0xFF00_0000,
            truncated: false,
            extent,
        }),
    );
    let mut ui = Ui::new(Box::new(env), tessera_ui::Vocab::default(), (1024, 768));
    if reader {
        ui.set_text_layout_params(Some(Box::new(|_props, size, wrap| params(size, wrap))));
    }
    ui.set_font_ladders(vec![FontLadder::new(faces.to_vec())]);
    let window = ui.create_control(ControlKind::Window, 1).expect("window");
    ui.control_set_position(window, 16, 16);
    ui.control_set_size(window, LABEL.0 + 8, LABEL.1 + 8);
    ui.control_set_back_colour(window, Some([1.0, 0.10, 0.12, 0.20]));
    ui.control_set_visible(window, true);
    let label = ui.create_control(ControlKind::Label, 1).expect("label");
    ui.control_set_parent(label, Some(window)).expect("parent");
    ui.control_set_position(label, 4, 4);
    ui.control_set_size(label, LABEL.0, LABEL.1);
    ui.control_set_visible(label, true);
    ui.set_text(label, Some(PARAGRAPH.to_string()));
    Scene { ui, window }
}

fn settle(ui: &mut Ui) {
    let mut budget = Unlimited;
    let mut last = 1;
    for t in 0..8u64 {
        last = 16 * t + 1;
        ui.paint(Duration::from_millis(last), &mut budget);
        if ui.paint_settled() {
            break;
        }
    }
    // `ui-scaling.md` §9.3: every geometry preview settled before the capture.
    assert!(
        ui.capture_settle(Duration::from_millis(last), &mut budget, 256, |_| {}),
        "the capture settle did not finish: a golden would record a preview (§9.3 item 4)"
    );
    assert!(
        !ui.geometry_preview_pending(),
        "no surface in a geometry preview"
    );
}

/// The window's canvas as RGBA (its physical box, §6.1).
fn capture(ui: &mut Ui, k: ElementKey) -> (Vec<u8>, u32, u32) {
    settle(ui);
    let b = ui.physical_box(k);
    let (w, h) = (b.width().max(1), b.height().max(1));
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let p = ui
                .surface_pixel(k, x, y)
                .expect("the window owns a surface");
            data.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, p as u8, (p >> 24) as u8]);
        }
    }
    (data, w, h)
}

fn golden_path(stem: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join(format!("{stem}.png"))
}

fn write_png(path: &std::path::Path, data: &[u8], w: u32, h: u32) {
    let f = std::fs::File::create(path).expect("create png");
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), w, h);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    let mut writer = e.write_header().expect("png header");
    writer.write_image_data(data).expect("png data");
}

fn read_png(path: &std::path::Path) -> Option<(Vec<u8>, u32, u32)> {
    let f = std::fs::File::open(path).ok()?;
    let d = png::Decoder::new(std::io::BufReader::new(f));
    let mut r = d.read_info().ok()?;
    let mut buf = vec![0; r.output_buffer_size()?];
    let info = r.next_frame(&mut buf).ok()?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return None;
    }
    buf.truncate(info.buffer_size());
    Some((buf, info.width, info.height))
}

fn compare(stem: &str, img: &[u8], w: u32, h: u32) {
    let path = golden_path(stem);
    if std::env::var_os("TESSERA_BLESS").is_some_and(|v| v == "1") {
        std::fs::create_dir_all(path.parent().expect("dir")).expect("golden dir");
        write_png(&path, img, w, h);
        eprintln!("golden: blessed {}", path.display());
        return;
    }
    let Some((want, ww, wh)) = read_png(&path) else {
        panic!(
            "golden {} missing; run with TESSERA_BLESS=1 and check it by eye",
            path.display()
        );
    };
    assert_eq!((ww, wh), (w, h), "{stem}: golden size");
    let mut bad = 0usize;
    let mut first = None;
    for (i, (a, b)) in img.chunks(4).zip(want.chunks(4)).enumerate() {
        let d = a
            .iter()
            .zip(b)
            .map(|(x, y)| x.abs_diff(*y))
            .max()
            .unwrap_or(0);
        if d > 1 {
            bad += 1;
            first.get_or_insert((i % w as usize, i / w as usize, d));
        }
    }
    if bad > 0 {
        let actual = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{stem}.actual.png"));
        write_png(&actual, img, w, h);
        panic!(
            "{stem}: {bad} pixels differ (first {first:?}); actual at {}",
            actual.display()
        );
    }
}

#[test]
fn g2_painted_paragraph() {
    let faces = ladder();
    let heights: Vec<i32> = faces.iter().map(|f| f.line_height_i32()).collect();
    let base = heights[0];
    // The five scales reach the three glyph sources of §6.4.3 (a true oracle).
    let kinds: Vec<SourceKind> = SCALES
        .iter()
        .map(|&c| scaling::choose_glyph_source(&heights, scaling::target_height(base, c, 1.0)).kind)
        .collect();
    assert_eq!(
        kinds[0],
        SourceKind::Exact,
        "c = 1 is the record itself (I7)"
    );
    for k in [
        SourceKind::Exact,
        SourceKind::Downsampled,
        SourceKind::Distance,
    ] {
        assert!(
            kinds.contains(&k),
            "{k:?} not reached: {kinds:?} over {heights:?}"
        );
    }

    // The identity picture (no UI screen) for I2.
    let mut id = scene(&faces);
    let identity = capture(&mut id.ui, id.window);

    for &c in &SCALES {
        let mut s = scene(&faces);
        let wp = (1024.0 * c).round() as i32;
        let hp = (768.0 * c).round() as i32;
        let f = UiScreen::new(wp, hp, c).expect("screen");
        s.ui.set_ui_screen(f, scaling::safe_area(&f, None));
        assert_eq!(
            s.ui.screen_size(),
            (1024, 768),
            "c {c}: a 1024 × 768 UI screen"
        );
        let (img, w, h) = capture(&mut s.ui, s.window);
        let expect_w = ((f64::from(LABEL.0 + 8)) * c).round() as u32;
        assert_eq!(w, expect_w, "c {c}: the canvas is the physical box");
        // Some ink: the paragraph's light colour is present.
        assert!(
            img.chunks(4).any(|p| p[0] > 200 && p[1] > 200),
            "c {c}: no text pixels"
        );
        if c == 1.0 {
            assert_eq!(
                (img.as_slice(), w, h),
                (identity.0.as_slice(), identity.1, identity.2),
                "c = 1 with a UI screen is byte-identical to the identity (I2)"
            );
        }
        let stem = format!("uiscale_g2_c{}", (c * 100.0).round() as u32);
        compare(&stem, &img, w, h);
    }
}

/// G2 without a layout-parameter reader at *c* = 1.5: the scaled path keeps the shaper's line
/// breaks and lays each line out again in physical pixels with derived glyphs
/// (`relayout_lines`), the path a client takes before it registers a reader. Compared with
/// `tests/golden/uiscale_g2_noreader_c150.png`; at *c* = 1 it is the identity (I2).
#[test]
fn g2_painted_paragraph_without_a_reader() {
    let faces = ladder();
    let mut id = scene_with(&faces, false);
    let identity = capture(&mut id.ui, id.window);
    let mut one = scene_with(&faces, false);
    let f = UiScreen::new(1024, 768, 1.0).expect("screen");
    one.ui.set_ui_screen(f, scaling::safe_area(&f, None));
    assert_eq!(
        capture(&mut one.ui, one.window),
        identity,
        "c = 1 with a UI screen and no reader is the identity (I2)"
    );
    let c = 1.5;
    let mut s = scene_with(&faces, false);
    let f = UiScreen::new(1536, 1152, c).expect("screen");
    s.ui.set_ui_screen(f, scaling::safe_area(&f, None));
    let (img, w, h) = capture(&mut s.ui, s.window);
    assert_eq!(w, ((f64::from(LABEL.0 + 8)) * c).round() as u32);
    assert!(
        img.chunks(4).any(|p| p[0] > 200 && p[1] > 200),
        "no text pixels"
    );
    compare("uiscale_g2_noreader_c150", &img, w, h);
}
