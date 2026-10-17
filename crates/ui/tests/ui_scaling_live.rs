// SPDX-License-Identifier: GPL-3.0-or-later
//! The live layout + paint path of `ui-scaling.md` §3.6, §5 and §6 wired through the pure
//! `tessera_ui::scaling` core (decision 0040). `ui_scaling_invariants.rs` covers the stateless
//! core; this file drives a real [`Ui`] through [`Ui::set_ui_screen`] and asserts the §9.5
//! invariants that become reachable once the screen *F*, the physical-box mapping (§3.6), the
//! composition seam (§6.6), the degenerate guard (§5.9) and keep-on-screen (§5.8) run live:
//! I1 (a window change re-lays the roots into *F*, a round trip restores the boxes), I3/I4
//! (physical boxes equal the pixel-centre mapping of the displayed boxes and abut), I5 (no
//! degenerate box), I8/keep-on-screen (a restored placement is brought inside *F*), I9 (a scale
//! change yields a composition) and the **identity** I2/I10 (`Se` = 1 is byte-identical and the
//! screen matches the §9.2 table). All layout values are invented.

mod ui_common;

use std::time::Duration;

use tessera_formats::did::Did;
use tessera_ui::paint::{Rect, Unlimited};
use tessera_ui::scaling::{self, DBox, UiScreen};
use tessera_ui::{Ui, UiFrame};
use ui_common::*;

const LAYOUT: u32 = 0x22FF_5100;
const ROOT: u32 = 0x1000_5101;
const CHILD_A: u32 = 0x1000_5102;
const CHILD_B: u32 = 0x1000_5103;

/// A named §9.2 configuration: window, user scale, safe aspect (`None` = `"full"`), and the
/// resulting `Wu` × `Hu`.
struct Config {
    name: &'static str,
    wp: i32,
    hp: i32,
    s: f64,
    aspect: Option<f64>,
    wu: i32,
    hu: i32,
}

const A21: Option<f64> = Some(2.4);

// C2 (identity, Se = 1), C5 (3840×2160 @ 1.5), C8 (ultrawide 5120×1440 @ 1.0, A = 3456 at 832).
const C2: Config = Config {
    name: "C2",
    wp: 1920,
    hp: 1080,
    s: 1.0,
    aspect: A21,
    wu: 1920,
    hu: 1080,
};
const C5: Config = Config {
    name: "C5",
    wp: 3840,
    hp: 2160,
    s: 1.5,
    aspect: A21,
    wu: 2560,
    hu: 1440,
};
const C8: Config = Config {
    name: "C8",
    wp: 5120,
    hp: 1440,
    s: 1.0,
    aspect: A21,
    wu: 5120,
    hu: 1440,
};

fn screen_of(c: &Config) -> UiScreen {
    let se = scaling::effective_scale(c.s, c.wp, c.hp);
    UiScreen::new(c.wp, c.hp, se).expect("non-zero window")
}

/// A root (design 1024 × 768, edges 5/5/5/5, a tiled image so it owns a surface) with two
/// abutting `KEEP`-edge children, A at (10, 20, 100 × 50) and B at (110, 20, 100 × 50) (A's
/// exclusive right 110 = B's left).
fn build() -> Ui {
    let a = edges(field(CHILD_A, 10, 20, 100, 50), [0, 0, 0, 0]);
    let b = edges(field(CHILD_B, 110, 20, 100, 50), [0, 0, 0, 0]);
    let root = child(
        child(
            media(
                edges(field(ROOT, 0, 0, 1024, 768), [5, 5, 5, 5]),
                image_item(0x4100_5101, 2),
            ),
            a,
        ),
        b,
    );
    let mut env = env_with(vec![layout(LAYOUT, 1024, 768, vec![root])]);
    add_image(&mut env, 0x4100_5101, solid(8, 8, 0xFF20_3040));
    ui_with(env, 1024, 768)
}

fn open(
    ui: &mut Ui,
) -> (
    tessera_ui::ElementKey,
    tessera_ui::ElementKey,
    tessera_ui::ElementKey,
) {
    let r = ui.open_panel(Did(LAYOUT), ROOT).unwrap();
    let a = ui.find_child(r, CHILD_A).unwrap();
    let b = ui.find_child(r, CHILD_B).unwrap();
    (r, a, b)
}

fn frames(ui: &mut Ui, n: u64) -> UiFrame {
    let mut budget = Unlimited;
    let mut last = UiFrame::default();
    for t in 0..n {
        last = ui.paint(Duration::from_millis(16 * t + 1), &mut budget);
    }
    last
}

/// The physical box of a displayed box under a screen, by §3.6 "Box → physical" (half-open).
fn expect_physical(f: &UiScreen, d: Rect) -> Rect {
    Rect::new(
        f.edge_x(f64::from(d.x0)),
        f.edge_y(f64::from(d.y0)),
        f.edge_x(f64::from(d.x1)),
        f.edge_y(f64::from(d.y1)),
    )
}

/// I10 / the §9.2 table: `set_ui_screen` resizes the screen root to `Wu` × `Hu`, and the safe
/// area matches §5.1 (C8's centred 21:9 area, 3456 wide at 832).
#[test]
fn screen_matches_configuration_table() {
    for c in [&C2, &C5, &C8] {
        let mut ui = build();
        open(&mut ui);
        ui.set_ui_screen(screen_of(c), scaling::safe_area(&screen_of(c), c.aspect));
        assert_eq!(ui.screen_size(), (c.wu, c.hu), "{} F", c.name);
    }
    // C8: safe area 3456 wide at 832 (§5.1, §9.2).
    let f = screen_of(&C8);
    let a = scaling::safe_area(&f, A21);
    assert!((a.w - 3456.0).abs() < 1e-9, "C8 Aw {}", a.w);
    assert!((a.l - 832.0).abs() < 1e-9, "C8 Ax {}", a.l);
    // C2 (A = F): the safe area is the whole screen.
    let a2 = scaling::safe_area(&screen_of(&C2), A21);
    assert_eq!((a2.l, a2.w, a2.h), (0.0, 1920.0, 1080.0));
}

/// I2 identity: at `Se` = 1 with `A` = `F` (C2), the physical box of every element equals its
/// displayed box made a rectangle, and the composition destinations are unchanged — so every
/// existing golden and paint test (which never call `set_ui_screen`) stays byte-identical.
#[test]
fn identity_at_se_one() {
    let mut ui = build();
    let (r, a, b) = open(&mut ui);
    ui.set_ui_screen(screen_of(&C2), scaling::safe_area(&screen_of(&C2), A21));
    for k in [r, a, b] {
        assert_eq!(
            ui.physical_box(k),
            ui.displayed_box(k).to_rect(),
            "identity physical == displayed"
        );
    }
    let fr = frames(&mut ui, 4);
    assert!(!fr.composition.entries.is_empty());
    assert_eq!(
        fr.composition.entries[0].dest,
        ui.displayed_box(r).to_rect()
    );
}

/// I3 / I4: at scaled configurations the physical box equals the pixel-centre mapping of the
/// displayed box, the composition destination is that physical box (§6.6), and abutting
/// siblings (A's right = B's left) abut in physical pixels with no gap or overlap.
#[test]
fn physical_boxes_match_and_abut() {
    for c in [&C5, &C8] {
        let mut ui = build();
        let (r, a, b) = open(&mut ui);
        let f = screen_of(c);
        ui.set_ui_screen(f, scaling::safe_area(&f, c.aspect));
        for k in [r, a, b] {
            let want = expect_physical(&f, ui.displayed_box(k).to_rect());
            assert_eq!(ui.physical_box(k), want, "{} physical box of {k:?}", c.name);
        }
        // I4: A's exclusive right column equals B's left column.
        assert_eq!(
            ui.physical_box(a).x1,
            ui.physical_box(b).x0,
            "{} abut",
            c.name
        );
        // The whole scaled UI reaches the window edges (the root fills F → the window).
        let fr = frames(&mut ui, 4);
        assert!(!fr.composition.entries.is_empty(), "{} composition", c.name);
        assert_eq!(
            fr.composition.entries[0].dest,
            ui.physical_box(r),
            "{} root dest is its physical box",
            c.name
        );
        assert_eq!(
            ui.physical_box(r),
            Rect::new(0, 0, c.wp, c.hp),
            "{} full",
            c.name
        );
    }
}

/// §6.1 crisp canvases: a surface's canvas is allocated at the **content scale** (its physical
/// box in window pixels) and composed 1 : 1 onto that box, instead of a UI-unit canvas
/// GPU-upscaled. At `Se` = 2 the root's canvas is twice its UI-unit size; at `Se` = 1 (identity)
/// it is exactly the UI-unit size, byte-identical to before this increment (invariant I2).
#[test]
fn canvas_is_crisp_at_content_scale() {
    // Se = 1 (C2, 1920 × 1080): the canvas equals the root's displayed (UI-unit) box.
    let mut ui = build();
    let (r, _, _) = open(&mut ui);
    let f1 = screen_of(&C2);
    ui.set_ui_screen(f1, scaling::safe_area(&f1, A21));
    let fr1 = frames(&mut ui, 4);
    let d1 = ui.displayed_box(r).to_rect();
    assert_eq!(
        fr1.composition.entries[0].canvas,
        [d1.width(), d1.height()],
        "identity canvas = displayed (UI-unit) box"
    );
    assert_eq!(fr1.composition.entries[0].canvas, [1920, 1080]);

    // Se = 2 (3840 × 2160 @ S = 2.0): the canvas is the physical box, twice the UI-unit size,
    // and composes 1 : 1 onto its physical box.
    let mut ui = build();
    let (r, _, _) = open(&mut ui);
    let se2 = scaling::effective_scale(2.0, 3840, 2160);
    assert_eq!(se2, 2.0);
    let f2 = UiScreen::new(3840, 2160, se2).unwrap();
    assert_eq!(f2.ui_size(), (1920, 1080));
    ui.set_ui_screen(f2, scaling::safe_area(&f2, A21));
    let fr2 = frames(&mut ui, 4);
    let phys = ui.physical_box(r);
    assert_eq!(
        fr2.composition.entries[0].canvas,
        [phys.width(), phys.height()],
        "Se = 2 canvas = physical box"
    );
    assert_eq!(fr2.composition.entries[0].canvas, [3840, 2160]);
    let d2 = ui.displayed_box(r).to_rect();
    assert!(
        fr2.composition.entries[0].canvas[0] > d2.width()
            && fr2.composition.entries[0].canvas[1] > d2.height(),
        "Se = 2 canvas {:?} larger than the UI-unit box {}×{}",
        fr2.composition.entries[0].canvas,
        d2.width(),
        d2.height()
    );
    // 1 : 1: the composition destination is the physical box, equal in size to the canvas.
    assert_eq!(fr2.composition.entries[0].dest, phys);
    assert_eq!(
        [phys.width(), phys.height()],
        fr2.composition.entries[0].canvas
    );
}

/// I1 / I14 (live slice): a window change re-lays the roots into the new `F`, and a change back
/// to the first configuration restores every box exactly (no drift).
#[test]
fn round_trip_restores_boxes() {
    let mut ui = build();
    let (r, a, b) = open(&mut ui);
    let f2 = screen_of(&C2);
    ui.set_ui_screen(f2, scaling::safe_area(&f2, A21));
    let before: Vec<_> = [r, a, b].iter().map(|&k| ui.layout_box(k)).collect();
    // To C8 and back to C2.
    let f8 = screen_of(&C8);
    ui.set_ui_screen(f8, scaling::safe_area(&f8, A21));
    assert_eq!(ui.screen_size(), (C8.wu, C8.hu));
    ui.set_ui_screen(f2, scaling::safe_area(&f2, A21));
    let after: Vec<_> = [r, a, b].iter().map(|&k| ui.layout_box(k)).collect();
    assert_eq!(before, after, "C2 → C8 → C2 restores every layout box");
}

/// I5 (§5.7, §5.9 live): a child whose SAME right edge is carried past its KEEP left edge when
/// the root shrinks from a 2000-wide design into `F` keeps its **design** extent (never a
/// negative one) and lies inside `F`.
#[test]
fn degenerate_child_keeps_design_extent() {
    // Design 2000 × 768 root; a 51-wide child at (1900, 0), edges all SAME (5), so when the
    // root shrinks into F the right edge is carried left past the (design-fixed) left edge and
    // the computed width goes negative — the §5.9 guard must rescue it to its design extent.
    let kid = edges(field(CHILD_A, 1900, 0, 51, 20), [5, 5, 5, 5]);
    let root = child(edges(field(ROOT, 0, 0, 2000, 768), [5, 5, 5, 5]), kid);
    let env = env_with(vec![layout(LAYOUT, 2000, 768, vec![root])]);
    let mut ui = ui_with(env, 1024, 768);
    let rk = ui.open_panel(Did(LAYOUT), ROOT).unwrap();
    let ck = ui.find_child(rk, CHILD_A).unwrap();
    // C1: F = 1365 × 768, far narrower than the 2000 design width.
    let f = UiScreen::new(1280, 720, scaling::effective_scale(1.0, 1280, 720)).unwrap();
    ui.set_ui_screen(f, scaling::safe_area(&f, A21));
    let lb = ui.layout_box(ck).unwrap();
    assert!(lb.w() >= 1, "I5: width {} >= 1", lb.w());
    assert!(lb.h() >= 1, "I5: height {} >= 1", lb.h());
    assert_eq!(lb.w(), 51, "design extent kept");
    // Shifted inside F's UI width (1365).
    assert!(lb.l >= 0 && lb.r < f.wu(), "inside F: {:?}", lb);
}

/// I8 / keep-on-screen (§5.8 item 2, D): a placement saved near the right edge at a wide `F`,
/// applied at a narrower `F`, is brought back fully inside `F` by `scaling::keep_on_screen`.
#[test]
fn saved_placement_kept_on_screen() {
    // A fixed 200 × 100 window (KEEP edges), so it does not fill F and can overflow.
    let win = prop(
        edges(field(ROOT, 0, 0, 200, 100), [0, 0, 0, 0]),
        v().pids.save_loc,
        b(true),
    );
    let env = env_with(vec![layout(LAYOUT, 1024, 768, vec![win])]);
    let mut ui = ui_with(env, 1024, 768);
    let rk = ui.open_panel(Did(LAYOUT), ROOT).unwrap();
    // At C2 (F = 1920 × 1080) move it near the right edge and save.
    let f2 = screen_of(&C2);
    ui.set_ui_screen(f2, scaling::safe_area(&f2, A21));
    ui.move_to(rk, 1700, 40);
    assert_eq!(
        ui.displayed_box(rk),
        tessera_ui::geom::IBox::new(1700, 40, 1899, 139)
    );
    ui.record_saved(rk);
    // At a narrower F = 1536 × 864 (C3) the restored window would overflow; keep-on-screen
    // pulls it back so its right edge lies inside F.
    let f3 = UiScreen::new(1920, 1080, 1.25).unwrap();
    ui.set_ui_screen(f3, scaling::safe_area(&f3, A21));
    assert!(ui.apply_saved(rk));
    let d = ui.displayed_box(rk);
    assert!(d.r < f3.wu(), "right {} inside F {}", d.r, f3.wu());
    assert!(d.l >= 0 && d.t >= 0, "inside F top-left: {:?}", d);
}

/// The identity is reachable through the safe-area accessor too (`A` = `F` before any call and
/// at `Se` = 1), and a non-call UI is unchanged (invariant I2 for the whole existing suite).
#[test]
fn safe_area_default_is_full_screen() {
    let ui = build();
    assert_eq!(ui.ui_screen(), None);
    let a = ui.safe_area_box();
    assert_eq!(a, DBox::new(0.0, 0.0, 1024.0, 768.0));
}

// ----- §5.2–§5.6 live fit rules: ultrawide and tall windows --------------------------------

const LAYOUT2: u32 = 0x22FF_5200;
const FITROOT: u32 = 0x1000_5201;
const FITKID: u32 = 0x1000_5202;
const PANEL: u32 = 0x1000_5203;

/// A fit-root scene (`ui-scaling.md` §5.2, §5.4 Type A): a full-cover root with both horizontal
/// edges 6 over a 1024 × 768 design area, holding one child that covers it (so the content box
/// `C` is the whole area, which drives the magnification bound).
fn build_fit() -> Ui {
    let kid = edges(field(FITKID, 0, 0, 1024, 768), [0, 0, 0, 0]);
    let root = child(edges(field(FITROOT, 0, 0, 1024, 768), [6, 5, 6, 5]), kid);
    let env = env_with(vec![layout(LAYOUT2, 1024, 768, vec![root])]);
    ui_with(env, 1024, 768)
}

/// A safe-area panel scene (`ui-scaling.md` §5.2): a 200 × 100 HUD panel pinned to the right
/// (left edge OPPOSITE, right edge SAME, top SAME, bottom KEEP) in a 1024 × 768 design area. It
/// does not cover the area, so it is a safe-area panel, not a full-screen root.
fn build_safe_panel() -> Ui {
    let root = edges(field(PANEL, 824, 50, 200, 100), [4, 5, 5, 0]);
    let env = env_with(vec![layout(LAYOUT2, 1024, 768, vec![root])]);
    ui_with(env, 1024, 768)
}

/// C8 (32:9 ultrawide, 5120 × 1440 @ 1.0): a fit root covers within the §5.4 magnification
/// bound — 1024 × 768 is magnified by f = 1.875 to 1920 × 1440 and centred (left wing 1600),
/// **not** stretched across the full 32:9 width. The ultrawide guarantee of decision 0040 §2.3
/// and invariant I17, driven live through the tree.
#[test]
fn fit_root_covers_within_bound_c8() {
    let mut ui = build_fit();
    let fr = ui.open_panel(Did(LAYOUT2), FITROOT).unwrap();
    let f = screen_of(&C8);
    ui.set_ui_screen(f, scaling::safe_area(&f, A21));
    let d = ui.displayed_box(fr);
    assert_eq!(d.l, 1600, "fit root centred (left wing 1600)");
    assert_eq!(d.t, 0, "fit root top");
    assert!((d.w() - 1920).abs() <= 2, "fit root width {} ~ 1920", d.w());
    assert!(
        d.w() < f.wu(),
        "fit root {} not stretched to the full {} width",
        d.w(),
        f.wu()
    );
}

/// C8: a safe-area panel anchors to the safe area's right edge (832 + 3456 = 4288), not the
/// screen's far edge (5120). The §5.2 safe-area frame on a 32:9 screen.
#[test]
fn safe_area_panel_anchors_to_safe_edge_c8() {
    let mut ui = build_safe_panel();
    let p = ui.open_panel(Did(LAYOUT2), PANEL).unwrap();
    let f = screen_of(&C8);
    ui.set_ui_screen(f, scaling::safe_area(&f, A21));
    let d = ui.displayed_box(p);
    assert_eq!(d.w(), 200, "panel width kept");
    assert_eq!(d.h(), 100, "panel height kept");
    assert_eq!(d.r + 1, 4288, "panel right at the safe-area edge");
    assert!(
        d.r + 1 < f.physical().0,
        "panel not at the screen's far edge"
    );
}

/// A tall 1024 × 1600 window @ Se = 1: the 21:9 safe area is wider than the window, so A = F and
/// nothing is constrained horizontally. The fit root fits by height (f = 1) and is letterboxed
/// top and bottom (centred at 416), never stretched to the 1600 height; the safe-area panel
/// spans to the full width (A = F).
#[test]
fn tall_window_fits_by_height() {
    let se = scaling::effective_scale(1.0, 1024, 1600);
    let f = UiScreen::new(1024, 1600, se).unwrap();
    assert_eq!(f.ui_size(), (1024, 1600));
    let a = scaling::safe_area(&f, A21);
    assert_eq!(
        a,
        DBox::new(0.0, 0.0, 1024.0, 1600.0),
        "A = F on a tall window"
    );

    let mut ui = build_fit();
    let fr = ui.open_panel(Did(LAYOUT2), FITROOT).unwrap();
    ui.set_ui_screen(f, a);
    let d = ui.displayed_box(fr);
    assert_eq!(d.w(), 1024, "fit root fills the width at f = 1");
    assert!((d.h() - 768).abs() <= 2, "fit root height {} ~ 768", d.h());
    assert_eq!(d.t, 416, "fit root centred vertically ((1600 - 768) / 2)");

    let mut ui = build_safe_panel();
    let p = ui.open_panel(Did(LAYOUT2), PANEL).unwrap();
    ui.set_ui_screen(f, a);
    let dp = ui.displayed_box(p);
    assert_eq!(dp.w(), 200, "panel width kept");
    assert_eq!(dp.r + 1, 1024, "A = F: panel at the full-width right edge");
}

/// C2 (Se = 1) and C1 (1280 × 720 @ Se = 0.9375) keep the full-screen (5, 5, 5, 5) root on the
/// plain path: it fills `F` and the window exactly, with no safe-area fit and no node scale, so
/// the default configurations behave exactly as before this increment (invariant I2 / C1).
#[test]
fn full_screen_root_unchanged_at_c1_and_c2() {
    // C2: A = F, Se = 1 — byte-identical to the layout box made absolute.
    let mut ui = build_safe_panel();
    let p = ui.open_panel(Did(LAYOUT2), PANEL).unwrap();
    let f2 = screen_of(&C2);
    ui.set_ui_screen(f2, scaling::safe_area(&f2, A21));
    let d = ui.displayed_box(p);
    assert_eq!(d.w(), 200);
    assert_eq!(
        d.r + 1,
        1920,
        "A = F at C2: panel at the screen's right edge"
    );

    // C1: a full-screen root fills F (1365 × 768) and maps to the whole window; children abut.
    let mut ui = build();
    let (r, a, b) = open(&mut ui);
    let f1 = UiScreen::new(1280, 720, scaling::effective_scale(1.0, 1280, 720)).unwrap();
    ui.set_ui_screen(f1, scaling::safe_area(&f1, A21));
    assert_eq!(
        ui.displayed_box(r),
        tessera_ui::geom::IBox::new(0, 0, f1.wu() - 1, f1.hu() - 1),
        "C1 full-screen root fills F"
    );
    assert_eq!(
        ui.physical_box(r),
        Rect::new(0, 0, 1280, 720),
        "C1 fills window"
    );
    assert_eq!(
        ui.physical_box(a).x1,
        ui.physical_box(b).x0,
        "C1 children still abut"
    );
}
