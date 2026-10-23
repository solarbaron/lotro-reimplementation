// SPDX-License-Identifier: GPL-3.0-or-later
//! The live crisp-text and art paths of `ui-scaling.md` §6 through `tessera-ui`'s paint step:
//! text at the identity is unchanged (I2), text at a content scale comes from derived glyphs
//! and counts toward the paint slice (§6.4, §6.5, S7), the text factor reaches only the text
//! §7.5 names, the text source of §6.4.3 item 4, the physical text hit test (§6.4.6), the art
//! filter setter (§6.3, §7.6) and the backdrop cover fit (§5.5). Synthetic content only.

mod ui_common;

use std::sync::Arc;
use std::time::Duration;

use tessera_formats::did::Did;
use tessera_material::ui_image::{PixelLayout, UiImage};
use tessera_ui::paint::{ArtFilter, Unlimited};
use tessera_ui::scaling::{self, UiScreen};
use tessera_ui::text::{
    CharListBuilder, DEFAULT_RANGES, FontFace, FontId, LayoutParams, TesseraFont, UiTextSource,
    layout as text_layout,
};
use tessera_ui::{Background, ControlKind, ElementKey, MemoryEnv, PreparedText, Ui, object_mode};
use ui_common::*;

const TEXT: &str = "Crisp text at every scale";
const IMG: u32 = 0x41E1_0000;

fn data_face(px: u32) -> Arc<FontFace> {
    let f = TesseraFont::bundled()
        .expect("bundled font")
        .rasterise(px, DEFAULT_RANGES)
        .expect("rasterise");
    Arc::new(
        FontFace::from_parts(
            FontId::Data(0x4200_0B00 + px),
            f.line_height(),
            f.baseline(),
            f.borders(),
            f.glyphs().to_vec(),
            Arc::clone(f.foreground()),
            None,
        )
        .expect("face"),
    )
}

fn params(size: (i32, i32), wrap: Option<i32>) -> LayoutParams {
    LayoutParams {
        width: wrap.unwrap_or(size.0),
        height: size.1,
        ..LayoutParams::default()
    }
}

fn env() -> MemoryEnv {
    let fonts = vec![Some(data_face(12))];
    let mut b = CharListBuilder::new();
    b.colour(0xFFFF_FFFF).push_str(TEXT);
    let chars = b.finish();
    let l = text_layout(&chars, &fonts, &params((200, 40), None), None).expect("layout");
    let extent = (l.content_width, l.content_height);
    let mut env = MemoryEnv::default();
    env.texts.insert(
        TEXT.to_string(),
        Arc::new(PreparedText {
            layout: l,
            chars,
            fonts,
            outline: false,
            outline_colour: 0xFF00_0000,
            truncated: false,
            extent,
        }),
    );
    // A 4 × 4 image for the art filter: every texel differs.
    let px = (0..16u32)
        .map(|i| 0xFF00_0000 | (i * 15) << 16 | (255 - i * 15) << 8)
        .collect();
    env.images.insert(
        tessera_ui::ImageRef::Did(IMG),
        Arc::new(UiImage::new(4, 4, 4, 4, PixelLayout::A8R8G8B8, px).expect("image")),
    );
    env
}

struct Scene {
    ui: Ui,
    window: ElementKey,
    label: ElementKey,
    art: ElementKey,
}

fn scene(reader: bool) -> Scene {
    let mut ui = Ui::new(Box::new(env()), tessera_ui::Vocab::default(), (1024, 768));
    if reader {
        ui.set_text_layout_params(Some(Box::new(|_p, size, wrap| params(size, wrap))));
    }
    let window = ui.create_control(ControlKind::Window, 1).expect("window");
    ui.control_set_position(window, 20, 20);
    ui.control_set_size(window, 220, 100);
    ui.control_set_back_colour(window, Some([1.0, 0.0, 0.0, 0.0]));
    ui.control_set_visible(window, true);
    let label = ui.create_control(ControlKind::Label, 1).expect("label");
    ui.control_set_parent(label, Some(window)).expect("parent");
    ui.control_set_position(label, 10, 10);
    ui.control_set_size(label, 200, 40);
    ui.control_set_visible(label, true);
    ui.set_text(label, Some(TEXT.to_string()));
    let art = ui.create_control(ControlKind::Control, 1).expect("art");
    ui.control_set_parent(art, Some(window)).expect("parent");
    ui.control_set_position(art, 10, 60);
    ui.control_set_size(art, 4, 4);
    ui.control_set_visible(art, true);
    ui.control_set_background(art, Background::Did(IMG));
    Scene {
        ui,
        window,
        label,
        art,
    }
}

fn screen(ui: &mut Ui, c: f64) {
    let f =
        UiScreen::new((1024.0 * c).round() as i32, (768.0 * c).round() as i32, c).expect("screen");
    ui.set_ui_screen(f, scaling::safe_area(&f, None));
}

fn settle(ui: &mut Ui) -> u32 {
    let mut budget = Unlimited;
    let mut derived = 0u32;
    for t in 0..8u64 {
        let fr = ui.paint(Duration::from_millis(16 * t + 1), &mut budget);
        derived += fr.stats.glyphs_derived;
        if ui.paint_settled() {
            break;
        }
    }
    derived
}

fn canvas(ui: &mut Ui, k: ElementKey) -> Vec<u32> {
    settle(ui);
    let b = ui.physical_box(k);
    let mut out = Vec::new();
    for y in 0..b.height() as i32 {
        for x in 0..b.width() as i32 {
            out.push(ui.surface_pixel(k, x, y).unwrap_or(0));
        }
    }
    out
}

/// I2: at `Se` = 1 with a UI screen set the text canvas is byte-identical to the identity,
/// with and without a layout-parameter reader.
#[test]
fn identity_text_is_unchanged() {
    for reader in [false, true] {
        let mut a = scene(reader);
        let base = canvas(&mut a.ui, a.window);
        let mut b = scene(reader);
        screen(&mut b.ui, 1.0);
        assert_eq!(canvas(&mut b.ui, b.window), base, "reader {reader}");
    }
}

/// §6.4, §6.5, S7: text at a content scale is drawn from derived glyphs (counted in the
/// frame's figures and kept in the glyph cache), with and without a reader; the canvas is the
/// physical box.
#[test]
fn scaled_text_derives_and_counts() {
    for reader in [false, true] {
        let mut s = scene(reader);
        screen(&mut s.ui, 1.5);
        let derived = settle(&mut s.ui);
        assert!(derived > 0, "reader {reader}: no glyph derived");
        let (held, bytes, total) = s.ui.glyph_cache_stats();
        assert!(held > 0 && bytes > 0 && total >= u64::from(derived));
        let b = s.ui.physical_box(s.window);
        assert_eq!(b.width(), 330);
        // A second paint of the unchanged UI derives nothing (cache and run reuse).
        s.ui.invalidate_paint();
        assert_eq!(settle(&mut s.ui), 0, "reader {reader}: derived again");
    }
}

/// §7.5: the text factor reaches only marked, tooltip and scrollable multi-line text, and only
/// with a reader; a plain label keeps `x` = 1.
#[test]
fn text_factor_reaches_only_reflowing_text() {
    let mut s = scene(true);
    let base = canvas(&mut s.ui, s.window);
    s.ui.set_text_scale(1.25);
    assert!((s.ui.text_scale() - 1.25).abs() < 1e-12);
    assert_eq!(
        canvas(&mut s.ui, s.window),
        base,
        "a plain label keeps x = 1"
    );
    s.ui.set_text_reflow(s.label, true);
    assert_ne!(canvas(&mut s.ui, s.window), base, "a marked label takes x");
    s.ui.set_text_reflow(s.label, false);
    assert_eq!(canvas(&mut s.ui, s.window), base);
    // Without a reader the factor is not applied (it needs a new layout).
    let mut n = scene(false);
    let base_n = canvas(&mut n.ui, n.window);
    n.ui.set_text_scale(1.5);
    n.ui.set_text_reflow(n.label, true);
    assert_eq!(canvas(&mut n.ui, n.window), base_n);
    // The setter clamps and rounds to the 0.05 steps of §7.1.
    n.ui.set_text_scale(9.0);
    assert!((n.ui.text_scale() - 1.5).abs() < 1e-12);
    n.ui.set_text_scale(1.024);
    assert!((n.ui.text_scale() - 1.0).abs() < 1e-12);
    n.ui.set_text_scale(f64::NAN);
    assert!((n.ui.text_scale() - 1.0).abs() < 1e-12);
}

/// §6.4.3 item 4: with `ui_text_source = "tessera"` the game fonts are replaced by the
/// bundled font rasterised at the target height; back to `"game"` the picture returns.
#[test]
fn text_source_switches_fonts() {
    let mut s = scene(true);
    let base = canvas(&mut s.ui, s.window);
    assert_eq!(s.ui.text_source(), UiTextSource::Game);
    s.ui.set_text_source(UiTextSource::Tessera);
    assert_ne!(canvas(&mut s.ui, s.window), base);
    s.ui.set_text_source(UiTextSource::Game);
    assert_eq!(canvas(&mut s.ui, s.window), base);
}

/// §6.4.6: the hit test works in physical pixels on the layout the text was drawn with.
#[test]
fn physical_hit_test() {
    for c in [1.0, 2.0] {
        let mut s = scene(true);
        screen(&mut s.ui, c);
        settle(&mut s.ui);
        let b = s.ui.physical_box(s.label);
        // Inside the first glyph cell of the first line.
        let y = b.y0 + (4.0 * c) as i32;
        assert_eq!(
            s.ui.text_hit_test(s.label, b.x0 + 1, y),
            Some(0),
            "c {c}: first character"
        );
        // Below the only line: no character.
        assert_eq!(
            s.ui.text_hit_test(s.label, b.x0 + 1, b.y1 - 1),
            None,
            "c {c}"
        );
        // Past the end of the text on its line: no character.
        assert_eq!(s.ui.text_hit_test(s.label, b.x1 - 1, y), None, "c {c}");
    }
}

/// §6.3, §7.6: the art filter setter repaints images at a scale; at the identity every filter
/// paints the same canvas.
#[test]
fn art_filter_setter() {
    let mut s = scene(false);
    let base = canvas(&mut s.ui, s.window);
    s.ui.set_art_filter(ArtFilter::Smooth);
    assert_eq!(s.ui.art_filter(), ArtFilter::Smooth);
    assert_eq!(canvas(&mut s.ui, s.window), base, "identity: exact copy");
    let mut t = scene(false);
    screen(&mut t.ui, 1.5);
    let sharp = canvas(&mut t.ui, t.window);
    t.ui.set_art_filter(ArtFilter::Smooth);
    let smooth = canvas(&mut t.ui, t.window);
    t.ui.set_art_filter(ArtFilter::Nearest);
    let nearest = canvas(&mut t.ui, t.window);
    assert_ne!(sharp, smooth);
    assert_ne!(smooth, nearest);
    let _ = t.art;
}

/// §5.5: a 1024 × 768 backdrop made for a Stretch root that the layout resized to 1365 × 768
/// is drawn as one cover-fit copy, not tiled.
#[test]
fn backdrop_image_covers_a_resized_stretch_element() {
    const LAYOUT: u32 = 0x2200_0E01;
    let (iw, ih) = (1024u32, 768u32);
    let px = (0..ih)
        .flat_map(|y| (0..iw).map(move |x| (x, y)))
        .map(|(x, y)| 0xFF00_0000 | ((x * 255 / 1023) << 16) | ((y * 255 / 767) << 8))
        .collect();
    let img = Arc::new(UiImage::new(iw, ih, iw, ih, PixelLayout::A8R8G8B8, px).expect("image"));
    let root = prop(
        media(
            edges(field(1, 0, 0, 1024, 768), [0, 0, 5, 5]),
            image_item(IMG + 1, 2),
        ),
        v().pids.object_mode,
        en(object_mode::STRETCH),
    );
    let mut env = env_with(vec![layout(LAYOUT, 1024, 768, vec![root])]);
    add_image(&mut env, IMG + 1, img);
    let mut ui = ui_with(env, 1365, 768);
    let k = ui.open_panel(Did(LAYOUT), 1).expect("panel");
    let b = ui.layout_box(k).expect("box");
    assert_eq!((b.w(), b.h()), (1365, 768), "the layout resized the root");
    settle(&mut ui);
    let at = |x: i32, y: i32| ui.surface_pixel(k, x, y).expect("surface");
    let red = |p: u32| (p >> 16) & 0xFF;
    let green = |p: u32| (p >> 8) & 0xFF;
    assert!(
        red(at(1364, 384)) >= 254,
        "the right end is the last column"
    );
    assert!(red(at(1100, 384)) > 200, "not tiled");
    assert!(
        green(at(10, 0)).abs_diff(32) <= 1,
        "centred: top row ≈ texel row 96"
    );
    assert!(
        green(at(10, 767)).abs_diff(223) <= 1,
        "bottom row ≈ texel row 671"
    );
}

/// The bounding box (x0, y0, x1, y1) of the white text ink in a canvas `w` pixels wide (the
/// synthetic image's texels have no blue, the window's back colour is black).
fn ink_box(px: &[u32], w: usize) -> Option<(usize, usize, usize, usize)> {
    let mut b: Option<(usize, usize, usize, usize)> = None;
    for (i, p) in px.iter().enumerate() {
        let white = (p & 0xFF) > 128 && ((p >> 8) & 0xFF) > 128 && ((p >> 16) & 0xFF) > 128;
        if white {
            let (x, y) = (i % w, i / w);
            b = Some(match b {
                None => (x, y, x + 1, y + 1),
                Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1)),
            });
        }
    }
    b
}

/// §6.1, §6.2, §5.5 last paragraph: a stretch vector is a picture stretch of the whole
/// surface, text included. Under a UI screen at `Se` = 1, text in a surface stretched by
/// (2, 1) is twice as wide and as tall as before, and by (1, 2) as wide and twice as tall;
/// the hit test takes the pointer back by σ (§6.4.6).
#[test]
fn stretched_surface_text_stretches() {
    let mut base = scene(true);
    screen(&mut base.ui, 1.0);
    let bw = base.ui.physical_box(base.window).width() as usize;
    let a = canvas(&mut base.ui, base.window);
    let (ax0, ay0, ax1, ay1) = ink_box(&a, bw).expect("ink");
    for sigma in [[2.0f32, 1.0], [1.0, 2.0]] {
        let mut s = scene(true);
        screen(&mut s.ui, 1.0);
        s.ui.set_node_scale(s.window, Some([sigma[0], sigma[1], 1.0]));
        let sw = s.ui.physical_box(s.window).width() as usize;
        assert!(
            sw.abs_diff(bw * sigma[0] as usize) <= 1,
            "{sigma:?}: canvas width {sw}"
        );
        let b = canvas(&mut s.ui, s.window);
        let (x0, y0, x1, y1) = ink_box(&b, sw).expect("ink");
        let (fx, fy) = (sigma[0] as usize, sigma[1] as usize);
        assert!(
            (x1 - x0).abs_diff((ax1 - ax0) * fx) <= 2,
            "{sigma:?}: ink width {} vs {}",
            x1 - x0,
            ax1 - ax0
        );
        assert!(
            (y1 - y0).abs_diff((ay1 - ay0) * fy) <= 2,
            "{sigma:?}: ink height {} vs {}",
            y1 - y0,
            ay1 - ay0
        );
        assert!(x0.abs_diff(ax0 * fx) <= 2, "{sigma:?}: ink left");
        // Hit test: the same character under the same point of the stretched picture.
        let pb = base.ui.physical_box(base.label);
        let ps = s.ui.physical_box(s.label);
        for dx in [1i32, 15, 40, 80, 120] {
            let dy = 5i32;
            let want = base.ui.text_hit_test(base.label, pb.x0 + dx, pb.y0 + dy);
            let got = s.ui.text_hit_test(
                s.label,
                ps.x0 + dx * sigma[0] as i32,
                ps.y0 + dy * sigma[1] as i32,
            );
            assert_eq!(got, want, "{sigma:?}: hit at {dx}");
        }
    }
}

/// §6.4.5, §7.5: with a reader, `Se` = 1.5 and the text factor 1.25, the plain tooltip is
/// sized from its text laid out at c × x: its width grows by the scaled extent (physical
/// extent ÷ c, rounded up), and the text element's box holds that extent (not clipped).
#[test]
fn tooltip_is_sized_from_its_scaled_text() {
    use tessera_formats::property::PropertyValue;
    use tessera_formats::string_info::{StringInfo, StringSource};
    use tessera_ui::text::{ScaleCtx, ceil_div_scale, layout_scaled, physical_params};
    const TIP: &str = "A tooltip text";
    const LAYOUT: u32 = 0x22FF_3000;
    let tip_layout = 0x22FF_3001;
    let vv = v();
    let pp = vv.pids;
    let (tt, txt) = (0x1000_0060, 0x1000_0061);
    let text = z(
        edges(typed(txt, vv.types.text, 2, 2, 26, 26), [5, 5, 5, 5]),
        0,
        2,
    );
    let panel = prop(field(tt, 0, 0, 30, 30), pp.tooltip_text_id, en(txt));
    let panel = z(child(panel, text), -1500, 0);
    let literal = PropertyValue::StringInfo(Box::new(StringInfo {
        source: StringSource::Literal(TIP.into()),
        args: None,
    }));
    let mut e = field(0x1, 100, 100, 100, 100);
    e = prop(e, pp.tooltips_on, b(true));
    e = prop(e, pp.tooltip_entry, literal);
    e = prop(e, pp.tooltip_id, en(tt));
    e = prop(e, pp.tooltip_layout, did(tip_layout));
    let fonts = vec![Some(data_face(12))];
    let mut cb = CharListBuilder::new();
    cb.colour(0xFFFF_FFFF).push_str(TIP);
    let chars = cb.finish();
    let shaped = text_layout(&chars, &fonts, &params((26, 26), Some(1024)), None).expect("layout");
    let show = |x: f64| -> (i32, i32) {
        let mut env = env_with(vec![
            layout(LAYOUT, 1024, 768, vec![e.clone()]),
            layout(tip_layout, 1024, 768, vec![panel.clone()]),
        ]);
        env.texts.insert(
            TIP.into(),
            Arc::new(PreparedText {
                layout: shaped.clone(),
                chars: chars.clone(),
                fonts: fonts.clone(),
                outline: false,
                outline_colour: 0xFF00_0000,
                truncated: false,
                extent: (shaped.content_width, shaped.content_height),
            }),
        );
        let mut ui = ui_with(env, 1024, 768);
        ui.set_text_layout_params(Some(Box::new(|_p, size, wrap| params(size, wrap))));
        screen(&mut ui, 1.5);
        ui.set_text_scale(x);
        let k = ui.open_panel(Did(LAYOUT), 0x1).expect("panel");
        let _ = k;
        ui.pointer_moved(150, 150);
        ui.set_time(1.0);
        ui.tick_hover();
        let t = ui.tooltip_panel().expect("tooltip shown");
        let pb = ui.layout_box(t).expect("box");
        let te = ui.find_descendant(t, txt).expect("text element");
        let tb = ui.layout_box(te).expect("box");
        (pb.w(), tb.w())
    };
    // The expected extents, computed independently: laid out at c × x in physical pixels.
    let ext = |x: f64| -> i32 {
        let map = |v: i32| (f64::from(v) * 1.5).round() as i32;
        let pp = physical_params(&params((26, 26), Some(1024)), map, map, 1.5);
        let l = layout_scaled(
            &chars,
            &fonts,
            &pp,
            None,
            &ScaleCtx {
                c: 1.5,
                x,
                ladders: &[],
            },
        )
        .expect("layout");
        ceil_div_scale(l.content_width, 1.5)
    };
    let (w1, _) = show(1.0);
    let (w2, tw2) = show(1.25);
    assert!(ext(1.25) > ext(1.0));
    assert_eq!(
        w2 - w1,
        ext(1.25) - ext(1.0),
        "the panel grows by the scaled extent"
    );
    assert!(tw2 >= ext(1.25), "the text box holds the text: {tw2}");
}

fn perf() -> bool {
    std::env::var_os("TESSERA_PERF").is_some_and(|v| v == "1")
}

/// §8 S7: every frame reports the time of its glyph derivations on the frame thread (their sum
/// and the longest one) next to their count, so a quiet-machine run can check "at most 20 µs per
/// glyph". The bound itself is asserted only with `TESSERA_PERF=1` (`docs/DEVELOPMENT.md`).
#[test]
fn glyph_derivation_times_are_reported() {
    let mut s = scene(true);
    screen(&mut s.ui, 1.5);
    let mut budget = Unlimited;
    let (mut glyphs, mut ns, mut max) = (0u64, 0u64, 0u64);
    for t in 0..8u64 {
        let fr = s.ui.paint(Duration::from_millis(16 * t + 1), &mut budget);
        let st = fr.stats;
        assert!(st.glyph_derivation_max_ns <= st.glyph_derivation_ns);
        if st.glyphs_derived == 0 {
            assert_eq!(st.glyph_derivation_ns, 0, "no derivation, no time");
        }
        glyphs += u64::from(st.glyphs_derived);
        ns += st.glyph_derivation_ns;
        max = max.max(st.glyph_derivation_max_ns);
        if s.ui.paint_settled() {
            break;
        }
    }
    assert!(glyphs > 0);
    eprintln!(
        "S7: {glyphs} glyphs derived on the frame thread, mean {} ns, longest {max} ns",
        ns / glyphs
    );
    if perf() {
        assert!(
            ns / glyphs <= 20_000,
            "S7: mean {} ns per glyph",
            ns / glyphs
        );
    }
}

/// §6.5, §8 S7: a ladder step (the printable ASCII range of a ladder at one target height) is
/// derived by a pure function meant for the workers, timed there; the UI takes its glyphs into
/// the cache, reports the step's time in the next frame's figures, and the drawing path then
/// finds those glyphs instead of deriving them again.
#[test]
fn a_ladder_step_is_derived_off_the_frame_thread_and_reported() {
    use tessera_ui::text::{FontLadder, LADDER_STEP_UNITS, derive_ladder_step};
    let face = data_face(12);
    // c = 1.5: the label's target height, the distance method from the one record.
    let t = scaling::target_height(face.line_height_i32(), 1.5, 1.0);
    let ladder = FontLadder::new(vec![face]);
    let step = std::thread::spawn(move || derive_ladder_step(&ladder, t))
        .join()
        .expect("worker");
    let units = LADDER_STEP_UNITS.count();
    assert!(!step.glyphs.is_empty() && step.glyphs.len() <= units);
    let elapsed = step.elapsed;
    let mut s = scene(true);
    screen(&mut s.ui, 1.5);
    s.ui.accept_ladder_step(step);
    let (held, _, before) = s.ui.glyph_cache_stats();
    assert!(held > 0);
    let fr = s.ui.paint(Duration::from_millis(1), &mut Unlimited);
    assert_eq!(fr.stats.ladder_steps, 1);
    assert_eq!(
        fr.stats.ladder_step_ns,
        u64::try_from(elapsed.as_nanos()).unwrap()
    );
    assert_eq!(fr.stats.ladder_step_max_ns, fr.stats.ladder_step_ns);
    let mut derived = fr.stats.glyphs_derived;
    derived += settle(&mut s.ui);
    assert_eq!(derived, 0, "the label's glyphs came from the step");
    assert_eq!(s.ui.glyph_cache_stats().2, before, "nothing derived again");
    let fr = s.ui.paint(Duration::from_millis(200), &mut Unlimited);
    assert_eq!(fr.stats.ladder_steps, 0, "reported once");
    eprintln!("S7: one ladder step of {} glyphs in {elapsed:?}", held);
    if perf() {
        assert!(
            elapsed <= Duration::from_millis(5),
            "S7: ladder step {elapsed:?}"
        );
    }
    // A target the ladder holds exactly derives nothing.
    let face = data_face(12);
    let lh = face.line_height_i32();
    let exact = derive_ladder_step(&FontLadder::new(vec![face]), lh);
    assert!(exact.glyphs.is_empty());
}

/// §8 S6 (revision 8 answer): one total *T* = `image_lru_mib` × max(1, *Se*²), the glyph caches
/// one eighth of it in whole MiB, the images the rest; set again with every change of *Se*, and
/// the glyph cache evicts down to its share when *Se* falls.
#[test]
fn the_s6_total_is_split_and_followed_by_the_caches() {
    use tessera_ui::scaling::glyph::{DerivedGlyph, GlyphKey, GlyphMetrics, SourceKind};
    use tessera_ui::scaling::s6_budget;
    use tessera_ui::text::LadderStep;
    const MIB: u64 = 1 << 20;
    let mut s = scene(true);
    // Before the host sets `image_lru_mib` the glyph share is the host's own.
    s.ui.set_glyph_cache_budget(3 << 20);
    screen(&mut s.ui, 2.0);
    assert_eq!(s.ui.glyph_memory().0, 3 << 20, "not overridden");
    s.ui.set_image_memory(64);
    let b = s.ui.s6_budget();
    assert_eq!(
        (b.total, b.glyphs, b.images),
        (256 * MIB, 32 * MIB, 224 * MIB)
    );
    assert_eq!(b, s6_budget(64, 2.0));
    assert_eq!(s.ui.glyph_memory().0 as u64, 32 * MIB);
    // Fill the derived-glyph cache with about 20 MiB.
    let glyph = |i: u32| {
        (
            GlyphKey {
                face: FontId::Data(0x4200_0001),
                target_height: 40,
                source: SourceKind::Distance,
                glyph: i,
                outline: false,
            },
            DerivedGlyph {
                metrics: GlyphMetrics::default(),
                coverage: vec![0x80; 1 << 20],
            },
        )
    };
    s.ui.accept_ladder_step(LadderStep {
        glyphs: (0..20).map(glyph).collect(),
        elapsed: Duration::ZERO,
    });
    let held = s.ui.glyph_memory().1 as u64;
    assert!(held > 19 * MIB && held <= 32 * MIB, "{held}");
    // Se falls to 1: T = 64 MiB, glyphs 8 MiB; the cache evicts down to it.
    screen(&mut s.ui, 1.0);
    let b = s.ui.s6_budget();
    assert_eq!((b.glyphs, b.images), (8 * MIB, 56 * MIB));
    let (share, held, _faces) = s.ui.glyph_memory();
    assert_eq!(share as u64, 8 * MIB);
    assert!(held as u64 <= 8 * MIB, "evicted to its share: {held}");
    // A user value replaces the 64 MiB.
    s.ui.set_image_memory(128);
    assert_eq!(s.ui.s6_budget().glyphs, 16 * MIB);
    // Revision 11: a small total keeps the 1 MiB glyph floor, taken from the images.
    screen(&mut s.ui, 1.0);
    s.ui.set_image_memory(4);
    let small = s.ui.s6_budget();
    assert_eq!((small.glyphs, small.images), (MIB, 3 * MIB));
    assert_eq!(s.ui.glyph_memory().0 as u64, MIB);
    s.ui.set_image_memory(128);
    // The image share is for the host's decoded-image LRU: tessera-ui holds no image cache of
    // its own and does not apply it (the host does). This only checks that an `ImageCache`
    // given the share evicts down to it.
    let mut cache = tessera_ui::paint::ImageCache::new(b.images, 512 * MIB);
    let img = || {
        UiImage::new(
            1024,
            1024,
            1024,
            1024,
            PixelLayout::A8R8G8B8,
            vec![0; 1 << 20],
        )
        .expect("image")
    };
    for i in 0..10 {
        cache
            .insert(tessera_ui::paint::ImageKey::Did(0x4100_0000 + i), img())
            .expect("insert");
    }
    assert_eq!(cache.stats().unpinned_bytes, 40 * MIB);
    cache.set_lru_budget(16 * MIB);
    assert!(cache.stats().unpinned_bytes <= 16 * MIB);
    assert_eq!(cache.lru_budget(), 16 * MIB);
}
