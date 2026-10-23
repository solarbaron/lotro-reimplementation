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
//! screen matches the §9.2 table). The live §5 rules are driven too: multi-member Type A fit
//! groups against the §10.6 theatre table and across two tree levels (I16), the letterbox bars
//! and their hits (§5.4 step 6, I3, I6), §5.3 safe boxes with the §10.2 side panels, §5.6 over a
//! node scale and for plugin windows, §5.8 items 1, 3 and 5, a safe-aspect change at an unchanged
//! UI-screen size (I14), and the stateful I1 part 2 (zero sides, minimise and restore). All
//! layout values are invented.

mod ui_common;

use std::time::Duration;

use tessera_formats::did::Did;
use tessera_ui::paint::{BandBudget, Rect, Unlimited};
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

// ----- more §9.2 configurations ------------------------------------------------------------

const C1: Config = Config {
    name: "C1",
    wp: 1280,
    hp: 720,
    s: 1.0,
    aspect: A21,
    wu: 1365,
    hu: 768,
};
const C7: Config = Config {
    name: "C7",
    wp: 3440,
    hp: 1440,
    s: 1.0,
    aspect: A21,
    wu: 3440,
    hu: 1440,
};
const C9: Config = Config {
    name: "C9",
    wp: 5120,
    hp: 1440,
    s: 1.0,
    aspect: None,
    wu: 5120,
    hu: 1440,
};

/// Applies a configuration and guards its `Wu × Hu`.
fn apply(ui: &mut Ui, c: &Config) -> UiScreen {
    let f = screen_of(c);
    ui.set_ui_screen(f, scaling::safe_area(&f, c.aspect));
    assert_eq!(ui.screen_size(), (c.wu, c.hu), "{} screen F", c.name);
    f
}

/// Asserts `got` is within `tol` of `want`.
fn near(got: f64, want: f64, tol: f64, what: &str) {
    assert!(
        (got - want).abs() <= tol,
        "{what}: got {got}, want {want} (± {tol})"
    );
}

fn stretch(d: tessera_formats::ui_layout::ElementDesc) -> tessera_formats::ui_layout::ElementDesc {
    prop(
        d,
        v().pids.object_mode,
        en(tessera_ui::object_mode::STRETCH),
    )
}

// ----- §5.4 fit groups: the theatre layout of §10.6 (I16, I3, step 6) -----------------------

const THEATRE: u32 = 0x22FF_5300;
const STORY: u32 = 0x1000_5301;
const STORY_KID: u32 = 0x1000_5302;
const PROGRESS: u32 = 0x1000_5303;
const PROGRESS_KID: u32 = 0x1000_5304;

/// Two fit roots of one 1024 × 768 layout (`ui-scaling.md` §10.6): the story layer whose content
/// covers its whole design area and the progress layer with one 247 × 49 child at (411, 735).
fn build_theatre() -> Ui {
    let story = child(
        edges(field(STORY, 0, 0, 1024, 768), [6, 5, 6, 5]),
        edges(field(STORY_KID, 0, 0, 1024, 768), [0, 0, 0, 0]),
    );
    let progress = child(
        edges(field(PROGRESS, 0, 0, 1024, 768), [6, 5, 6, 5]),
        edges(field(PROGRESS_KID, 411, 735, 247, 49), [0, 0, 0, 0]),
    );
    let env = env_with(vec![layout(THEATRE, 1024, 768, vec![story, progress])]);
    ui_with(env, 1024, 768)
}

fn open_theatre(ui: &mut Ui) -> [tessera_ui::ElementKey; 3] {
    let s = ui.open_panel(Did(THEATRE), STORY).unwrap();
    let p = ui.open_panel(Did(THEATRE), PROGRESS).unwrap();
    let k = ui.find_child(p, PROGRESS_KID).unwrap();
    [s, p, k]
}

/// §10.6 table: one factor and one displayed box for both layers of the group, the progress
/// child at its design place in the shared picture (I16). Rows C1, C2, C7, C8, plus C9.
#[test]
fn theatre_fit_group_matches_table() {
    // (config, f, group left, child left, child top, child width)
    let rows: [(&Config, f64, f64, f64, f64, f64); 5] = [
        (&C1, 1.0, 170.5, 581.5, 735.0, 247.0),
        (&C2, 1.40625, 240.0, 817.97, 1033.59, 347.34),
        (&C7, 1.875, 760.0, 1530.63, 1378.13, 463.13),
        (&C8, 1.875, 1600.0, 2370.63, 1378.13, 463.13),
        (&C9, 1.875, 1600.0, 2370.63, 1378.13, 463.13),
    ];
    for (c, f, gl, cl, ct, cw) in rows {
        let mut ui = build_theatre();
        let [s, p, k] = open_theatre(&mut ui);
        apply(&mut ui, c);
        near(
            f64::from(ui.fit_factor(s)),
            f,
            1e-4,
            &format!("{} f", c.name),
        );
        assert_eq!(ui.fit_factor(s), ui.fit_factor(p), "{}: one factor", c.name);
        let ds = ui.displayed_box(s);
        assert_eq!(ds, ui.displayed_box(p), "{}: one displayed box", c.name);
        near(f64::from(ds.l), gl, 1.0, &format!("{} group left", c.name));
        assert_eq!(ds.t, 0, "{} group top", c.name);
        let dk = ui.displayed_box(k);
        near(f64::from(dk.l), cl, 1.5, &format!("{} child left", c.name));
        near(f64::from(dk.t), ct, 1.5, &format!("{} child top", c.name));
        near(
            f64::from(dk.w()),
            cw,
            1.5,
            &format!("{} child width", c.name),
        );
    }
}

/// I16: showing or hiding a member changes no member's box or factor, and a hidden member still
/// takes part in the group at every later layout (the progress layer alone would get the cover
/// factor 1.875 at C2, the group gives 1.4063).
#[test]
fn theatre_hiding_a_layer_moves_nothing() {
    let mut ui = build_theatre();
    let [s, p, _] = open_theatre(&mut ui);
    apply(&mut ui, &C2);
    let (bs, bp, f) = (ui.displayed_box(s), ui.displayed_box(p), ui.fit_factor(p));
    ui.set_shown(s, false);
    assert_eq!(
        ui.displayed_box(p),
        bp,
        "hiding the story layer moves nothing"
    );
    assert_eq!(ui.fit_factor(p), f);
    apply(&mut ui, &C8);
    apply(&mut ui, &C2);
    near(
        f64::from(ui.fit_factor(p)),
        1.40625,
        1e-4,
        "hidden member kept in group",
    );
    assert_eq!(ui.displayed_box(p), bp);
    ui.set_shown(s, true);
    assert_eq!(ui.displayed_box(s), bs, "showing it again moves nothing");
    assert_eq!(ui.displayed_box(p), bp);
}

/// §5.4 step 6 and I3: the group draws its letterbox bars once, below its lowest shown member,
/// and a pointer in a bar hits that root, never what lies behind. Members plus bars cover F (I6).
#[test]
fn theatre_letterbox_bars_and_hits() {
    let mut ui = build_theatre();
    let [s, p, _] = open_theatre(&mut ui);
    apply(&mut ui, &C2);
    let bars = ui.letterbox_bars();
    assert_eq!(bars.len(), 1, "one group, bars drawn once");
    assert_eq!(bars[0].below, s, "below the lowest shown member");
    let area: u64 = bars[0]
        .rects
        .iter()
        .map(|r| u64::from(r.width()) * u64::from(r.height()))
        .sum();
    assert_eq!(area, 2 * 240 * 1080, "bars 240 wide at each side");
    let member = ui.physical_box(s);
    let member_area = u64::from(member.width()) * u64::from(member.height());
    assert_eq!(
        area + member_area,
        1920 * 1080,
        "I6: members and bars cover F"
    );
    for r in &bars[0].rects {
        assert!(r.intersect(&member).is_empty(), "bars never under a member");
    }
    assert_eq!(
        ui.hit_test(100, 500),
        Some(s),
        "left bar hits the lower root"
    );
    assert_eq!(
        ui.hit_test(1800, 20),
        Some(s),
        "right bar hits the lower root"
    );
    // With the lower layer hidden, the bars move below the progress layer and hit it.
    ui.set_shown(s, false);
    let bars = ui.letterbox_bars();
    assert_eq!(bars.len(), 1);
    assert_eq!(bars[0].below, p);
    assert_eq!(ui.hit_test(100, 500), Some(p));
    // No shown member: no bars.
    ui.set_shown(p, false);
    assert!(
        ui.letterbox_bars().is_empty(),
        "a group with no shown member draws none"
    );
    // At the identity no bars exist at all.
    let mut ui = build_theatre();
    open_theatre(&mut ui);
    assert!(ui.letterbox_bars().is_empty());
}

// ----- a fit group across two tree levels (the SC13 map group) -------------------------------

const MAPL: u32 = 0x22FF_5400;
const MAPROOT: u32 = 0x1000_5401;
const DECO: u32 = 0x1000_5402;
const DECO_KID: u32 = 0x1000_5403;
const INNER: u32 = 0x1000_5404;
const MAPBG: u32 = 0x1000_5405;
const MAPBG_KID: u32 = 0x1000_5406;

/// A full-screen (5, 5, 5, 5) root holding a Type A decoration and, one level deeper inside a
/// full-cover (5, 5, 5, 5) element, a Type A map background: one fit group (§5.4 grouping rule,
/// like the map group of SC13).
fn build_map() -> Ui {
    let deco = child(
        stretch(edges(field(DECO, 0, 0, 1024, 768), [6, 5, 6, 5])),
        edges(field(DECO_KID, 0, 0, 1024, 768), [0, 0, 0, 0]),
    );
    let bg = child(
        stretch(edges(field(MAPBG, 0, 0, 1024, 768), [6, 5, 6, 5])),
        edges(field(MAPBG_KID, 100, 100, 50, 50), [0, 0, 0, 0]),
    );
    let inner = child(edges(field(INNER, 0, 0, 1024, 768), [5, 5, 5, 5]), bg);
    let root = child(
        child(edges(field(MAPROOT, 0, 0, 1024, 768), [5, 5, 5, 5]), deco),
        inner,
    );
    let env = env_with(vec![layout(MAPL, 1024, 768, vec![root])]);
    ui_with(env, 1024, 768)
}

#[test]
fn two_level_fit_group_shares_one_map() {
    let mut ui = build_map();
    let r = ui.open_panel(Did(MAPL), MAPROOT).unwrap();
    let deco = ui.find_child(r, DECO).unwrap();
    let bg = ui.find_descendant(r, MAPBG).unwrap();
    let bg_kid = ui.find_descendant(r, MAPBG_KID).unwrap();
    for c in [&C2, &C7, &C8] {
        apply(&mut ui, c);
        let f = ui.fit_factor(deco);
        assert_eq!(f, ui.fit_factor(bg), "{}: one factor across levels", c.name);
        assert_eq!(
            ui.displayed_box(deco),
            ui.displayed_box(bg),
            "{}: one map",
            c.name
        );
        // The background's small child alone would cover by itself; the group follows the
        // decoration's full content (the §10.6 numbers for a 1024 × 768 group).
        let want = if c.name == "C2" { 1.40625 } else { 1.875 };
        near(f64::from(f), want, 1e-4, &format!("{} group f", c.name));
        // I16: the same design point (100, 100) of the background maps through the one map.
        let d = ui.displayed_box(deco);
        near(
            f64::from(ui.displayed_box(bg_kid).l),
            f64::from(d.l) + 100.0 * f64::from(f),
            1.5,
            &format!("{} child x", c.name),
        );
        // Bars are drawn once, below the lower member, in the root's full box.
        let bars = ui.letterbox_bars();
        assert_eq!(bars.len(), 1, "{}: one group's bars", c.name);
        assert_eq!(bars[0].below, deco);
    }
}

// ----- §10.1: a Type A backdrop of a full-screen root (the loading screen) -------------------

const LSL: u32 = 0x22FF_5600;
const LSROOT: u32 = 0x1000_5601;
const LSIMAGE: u32 = 0x1000_5602;
const LSBG: u32 = 0x1000_5603;

const C12: Config = Config {
    name: "C12",
    wp: 3840,
    hp: 2160,
    s: 1.0,
    aspect: A21,
    wu: 3840,
    hu: 2160,
};

/// A 2560 × 1080 layout whose full-screen (5, 5, 5, 5) root holds a 2560 × 1080 Type A
/// container (a group of one laid out in the root's full box, §5.3) with one 1024 × 768 child at
/// (768, 312), the content box of `ui-scaling.md` §10.1.
fn build_loading() -> Ui {
    let image = child(
        stretch(edges(field(LSIMAGE, 0, 0, 2560, 1080), [6, 5, 6, 5])),
        edges(field(LSBG, 768, 312, 1024, 768), [0, 0, 0, 0]),
    );
    let root = child(edges(field(LSROOT, 0, 0, 2560, 1080), [5, 5, 5, 5]), image);
    let env = env_with(vec![layout(LSL, 2560, 1080, vec![root])]);
    ui_with(env, 2560, 1080)
}

/// §10.1 rows C1, C7, C8 and C12 (3840 × 2160 at 1.0), plus C9 (`"full"`: *G* = *F*, the bound
/// still limits *f*, so the C8 box): the image container's factor and displayed box, and at C8
/// and C9 the group's side bars, drawn below it, which with the image cover *F* (I6).
#[test]
fn loading_screen_matches_10_1() {
    // (config, f, left, top, width, height)
    let rows: [(&Config, f64, f64, f64, f64, f64); 5] = [
        (&C1, 0.7111, -227.72, 0.0, 1820.44, 768.0),
        (&C7, 1.3438, 0.0, -11.25, 3440.0, 1451.25),
        (&C8, 1.7778, 284.44, -480.0, 4551.11, 1920.0),
        (&C9, 1.7778, 284.44, -480.0, 4551.11, 1920.0),
        (&C12, 2.0, -640.0, 0.0, 5120.0, 2160.0),
    ];
    for (c, f, l, t, w, h) in rows {
        let mut ui = build_loading();
        let r = ui.open_panel(Did(LSL), LSROOT).unwrap();
        let img = ui.find_child(r, LSIMAGE).unwrap();
        apply(&mut ui, c);
        assert_eq!(
            ui.displayed_box(r),
            tessera_ui::IBox::new(0, 0, c.wu - 1, c.hu - 1),
            "{}: the root's box is F",
            c.name
        );
        near(
            f64::from(ui.fit_factor(img)),
            f,
            1e-4,
            &format!("{} f", c.name),
        );
        let d = ui.displayed_box(img);
        near(f64::from(d.l), l, 1.5, &format!("{} left", c.name));
        near(f64::from(d.t), t, 1.5, &format!("{} top", c.name));
        near(f64::from(d.w()), w, 1.5, &format!("{} width", c.name));
        near(f64::from(d.h()), h, 1.5, &format!("{} height", c.name));
        // §3.4: the real-valued displayed box is §10.1's to the table's two decimals.
        let real = ui.displayed_real(img);
        for (got, want, what) in [
            (real.l, l, "left"),
            (real.t, t, "top"),
            (real.w, w, "width"),
            (real.h, h, "height"),
        ] {
            near(got, want, 0.01, &format!("{} real {what}", c.name));
        }
        let bars = ui.letterbox_bars();
        assert!(bars.len() <= 1, "{}: at most one group's bars", c.name);
        let member = ui.physical_box(img).intersect(&Rect::new(0, 0, c.wp, c.hp));
        let area: u64 = bars
            .iter()
            .flat_map(|g| g.rects.iter())
            .map(|b| u64::from(b.width()) * u64::from(b.height()))
            .sum();
        assert_eq!(
            area + u64::from(member.width()) * u64::from(member.height()),
            u64::try_from(c.wp).unwrap() * u64::try_from(c.hp).unwrap(),
            "{}: I6, image and bars cover F",
            c.name
        );
        if let Some(g) = bars.first() {
            assert_eq!(g.below, img, "{}: below the image", c.name);
        }
        if l > 0.0 {
            // The bound limits f: bars at both sides, as wide as the image's left.
            let side: u64 = bars
                .iter()
                .flat_map(|g| g.rects.iter())
                .filter(|b| b.height() == u32::try_from(c.hp).unwrap())
                .map(|b| u64::from(b.width()))
                .sum();
            near(side as f64, 2.0 * l, 3.0, &format!("{} side bars", c.name));
        } else {
            // The image covers F: the displayed box follows the real-valued rule of §3.4
            // (§10.1 at C1: 768 units high), so no bar row or column is left (§5.9 answer).
            let left: Vec<_> = bars.iter().flat_map(|g| g.rects.iter()).collect();
            assert!(left.is_empty(), "{}: no bars, got {left:?}", c.name);
        }
    }
}

// ----- §5.3: children of a full-screen root, the character screen of §10.2 ------------------

const CSL: u32 = 0x22FF_5500;
const CSROOT: u32 = 0x1000_5501;
const CSVIEW: u32 = 0x1000_5502;
const CSLEFT: u32 = 0x1000_5503;
const CSRIGHT: u32 = 0x1000_5504;

fn build_charscreen() -> Ui {
    let view = edges(field(CSVIEW, 0, 0, 1440, 900), [5, 5, 5, 5]);
    let left = stretch(edges(field(CSLEFT, 0, 0, 500, 900), [5, 5, 6, 5]));
    let right = stretch(edges(field(CSRIGHT, 940, 0, 500, 900), [6, 5, 5, 5]));
    let mut root = edges(field(CSROOT, 0, 0, 1440, 900), [5, 5, 5, 5]);
    for c in [view, left, right] {
        root = child(root, c);
    }
    let env = env_with(vec![layout(CSL, 1440, 900, vec![root])]);
    ui_with(env, 1440, 900)
}

/// §10.2: the side panels sit at the safe area's edges, the full-cover backdrop spans F.
#[test]
fn character_screen_side_panels_at_safe_edges() {
    // (config, f, left panel l..r, right panel l..r)
    let rows: [(&Config, f64, f64, f64, f64, f64); 3] = [
        (&C1, 0.8533, 0.0, 426.67, 938.33, 1365.0),
        (&C7, 1.6, 0.0, 800.0, 2640.0, 3440.0),
        (&C8, 1.6, 832.0, 1632.0, 3488.0, 4288.0),
    ];
    for (c, f, ll, lr, rl, rr) in rows {
        let mut ui = build_charscreen();
        let r = ui.open_panel(Did(CSL), CSROOT).unwrap();
        let view = ui.find_child(r, CSVIEW).unwrap();
        let left = ui.find_child(r, CSLEFT).unwrap();
        let right = ui.find_child(r, CSRIGHT).unwrap();
        apply(&mut ui, c);
        near(
            f64::from(ui.fit_factor(left)),
            f,
            1e-3,
            &format!("{} f", c.name),
        );
        let (dl, dr) = (ui.displayed_box(left), ui.displayed_box(right));
        near(f64::from(dl.l), ll, 1.0, &format!("{} left l", c.name));
        near(f64::from(dl.r + 1), lr, 1.0, &format!("{} left r", c.name));
        near(f64::from(dr.l), rl, 1.0, &format!("{} right l", c.name));
        near(f64::from(dr.r + 1), rr, 1.0, &format!("{} right r", c.name));
        assert_eq!(
            ui.displayed_box(view),
            tessera_ui::IBox::new(0, 0, c.wu - 1, c.hu - 1),
            "{}: the backdrop spans F",
            c.name
        );
    }
}

// ----- §5.6: fit to screen over a node scale and for plugin windows --------------------------

#[test]
fn fit_to_screen_multiplies_a_node_scale() {
    let win = edges(field(ROOT, 12, 12, 1000, 600), [0, 0, 0, 0]);
    let env = env_with(vec![layout(LAYOUT, 1024, 768, vec![win])]);
    let mut ui = ui_with(env, 1024, 768);
    let k = ui.open_panel(Did(LAYOUT), ROOT).unwrap();
    apply(&mut ui, &C2);
    ui.set_node_scale(k, Some([1.5, 1.5, 1.0]));
    assert_eq!(ui.fit_to_screen_factor(k), 1.0, "1500 × 900 fits C2");
    apply(&mut ui, &C1);
    let q = f64::from(ui.fit_to_screen_factor(k));
    near(q, 768.0 / 900.0, 1e-4, "q over the 1.5 scale");
    let d = ui.displayed_box(k);
    assert!(d.w() <= 1365 && d.h() <= 768, "shrunk to fit: {d:?}");
    assert!(
        d.l >= 0 && d.t >= 0 && d.r < 1365 && d.b < 768,
        "kept on screen"
    );
    assert_eq!(ui.layout_box(k).unwrap().w(), 1000, "layout size unchanged");
    apply(&mut ui, &C2);
    assert_eq!(ui.fit_to_screen_factor(k), 1.0, "q is 1 again once it fits");
    assert_eq!(ui.node_scale_at_or_above(k).0, 1.5, "the own scale is back");
}

#[test]
fn plugin_window_fit_to_screen_keeps_get_size() {
    let mut ui = build();
    apply(&mut ui, &C2);
    let w = ui
        .create_control(tessera_ui::ControlKind::Window, 1)
        .unwrap();
    ui.control_set_size(w, 1600, 900);
    ui.control_set_position(w, 100, 50);
    ui.control_set_visible(w, true);
    assert_eq!(ui.fit_to_screen_factor(w), 1.0);
    apply(&mut ui, &C1);
    near(
        f64::from(ui.fit_to_screen_factor(w)),
        (1365.0f64 / 1600.0).min(768.0 / 900.0),
        1e-4,
        "plugin q",
    );
    assert_eq!(ui.control_size(w), (1600, 900), "GetSize() unchanged");
    let d = ui.displayed_box(w);
    assert!(
        d.l >= 0 && d.t >= 0 && d.r < 1365 && d.b < 768,
        "I6: a plugin window inside the old F stays inside the new one: {d:?}"
    );
}

// ----- §5.8 items 1, 3 and 5: keep-on-screen after every change -----------------------------

#[test]
fn plugin_windows_follow_item_five() {
    let mut ui = build();
    apply(&mut ui, &C2);
    let inside = ui
        .create_control(tessera_ui::ControlKind::Window, 1)
        .unwrap();
    ui.control_set_size(inside, 300, 200);
    ui.control_set_position(inside, 1500, 800);
    ui.control_set_visible(inside, true);
    let off = ui
        .create_control(tessera_ui::ControlKind::Window, 1)
        .unwrap();
    ui.control_set_size(off, 300, 200);
    ui.control_set_position(off, 1800, 100);
    ui.control_set_visible(off, true);
    apply(&mut ui, &C1);
    let d = ui.displayed_box(inside);
    assert!(d.r < 1365 && d.b < 768, "inside stays inside: {d:?}");
    assert_eq!(
        ui.control_position(off),
        (1800, 100),
        "a window placed off screen with SetPosition is left alone"
    );
}

#[test]
fn native_root_kept_on_screen_after_screen_change() {
    let win = edges(field(ROOT, 0, 0, 300, 200), [0, 0, 0, 0]);
    let env = env_with(vec![layout(LAYOUT, 1024, 768, vec![win])]);
    let mut ui = ui_with(env, 1024, 768);
    let k = ui.open_panel(Did(LAYOUT), ROOT).unwrap();
    apply(&mut ui, &C2);
    ui.move_to(k, 1600, 850);
    apply(&mut ui, &C1);
    let d = ui.displayed_box(k);
    assert!(
        d.l >= 0 && d.t >= 0 && d.r < 1365 && d.b < 768,
        "I6: native root inside F: {d:?}"
    );
}

// ----- I14: a safe-aspect change at the same UI-screen size ---------------------------------

#[test]
fn safe_aspect_change_relays_out_c8_c9_c8() {
    let mut ui = build_safe_panel();
    let p = ui.open_panel(Did(LAYOUT2), PANEL).unwrap();
    apply(&mut ui, &C8);
    let b8 = ui.layout_box(p).unwrap();
    assert_eq!(ui.displayed_box(p).r + 1, 4288);
    apply(&mut ui, &C9);
    assert_eq!(ui.displayed_box(p).r + 1, 5120, "C9: at the screen's edge");
    apply(&mut ui, &C8);
    assert_eq!(ui.displayed_box(p).r + 1, 4288, "back at the safe edge");
    assert_eq!(ui.layout_box(p).unwrap(), b8, "I14: same layout box");
}

#[test]
fn dragged_mode_zero_panel_round_trips() {
    let root = edges(field(PANEL, 100, 100, 200, 100), [0, 0, 0, 0]);
    let env = env_with(vec![layout(LAYOUT2, 1024, 768, vec![root])]);
    let mut ui = ui_with(env, 1024, 768);
    let p = ui.open_panel(Did(LAYOUT2), PANEL).unwrap();
    apply(&mut ui, &C8);
    assert_eq!(ui.layout_box(p).unwrap().l, 932, "§5.2 example: 100 + 832");
    ui.move_to(p, 1500, 300);
    let dragged = ui.layout_box(p).unwrap();
    apply(&mut ui, &C9);
    assert_eq!(
        ui.layout_box(p).unwrap().l,
        1500 - 832,
        "frame distance kept"
    );
    apply(&mut ui, &C8);
    assert_eq!(ui.layout_box(p).unwrap(), dragged, "I14: round trip");
    // Laying the same configuration out twice gives the same boxes.
    apply(&mut ui, &C8);
    assert_eq!(ui.layout_box(p).unwrap(), dragged);
}

// ----- I1 part 2: zero sides and minimise / restore -----------------------------------------

/// Every box, factor and displayed box of a few live roots, for before/after comparisons.
fn snapshot(ui: &Ui, keys: &[tessera_ui::ElementKey]) -> Vec<(tessera_ui::IBox, f32, f32)> {
    keys.iter()
        .map(|k| {
            (
                ui.displayed_box(*k),
                ui.fit_factor(*k),
                ui.fit_to_screen_factor(*k),
            )
        })
        .collect()
}

#[test]
fn zero_side_and_minimise_restore_change_nothing() {
    let mut ui = build_theatre();
    let [s, p, k] = open_theatre(&mut ui);
    let panel = {
        let w = ui
            .create_control(tessera_ui::ControlKind::Window, 1)
            .unwrap();
        ui.control_set_size(w, 300, 200);
        ui.control_set_position(w, 4900, 100); // partly off screen at C8
        ui.control_set_visible(w, true);
        w
    };
    let f8 = apply(&mut ui, &C8);
    let keys = [s, p, k, panel];
    let before = snapshot(&ui, &keys);
    let (scr, safe) = (ui.ui_screen(), ui.safe_area_box());
    for (w, h) in [(0, 1440), (5120, 0), (0, 0)] {
        assert!(
            UiScreen::new(w, h, 1.0).is_none(),
            "a zero side has no screen"
        );
        ui.resize_screen(w, h);
        assert_eq!(ui.ui_screen(), scr, "Se and F kept");
        assert_eq!(ui.safe_area_box(), safe, "A kept");
        assert_eq!(ui.screen_size(), (5120, 1440));
        assert_eq!(snapshot(&ui, &keys), before, "boxes and factors kept");
    }
    // Restore to the same size: same Se, F, A, so nothing is laid out again and nothing moves.
    ui.set_ui_screen(f8, scaling::safe_area(&f8, A21));
    assert_eq!(
        snapshot(&ui, &keys),
        before,
        "minimise then restore moves nothing"
    );
    assert_eq!(ui.control_position(panel), (4900, 100));
}

// ----- review regressions: fits under a node scale (§3.4 item 2, §10.8 (b)) ------------------

const L108: u32 = 0x22FF_5800;
const R108: u32 = 0x1000_5801;
const X108: u32 = 0x1000_5802;
const K108: u32 = 0x1000_5803;

/// §10.8 (b): a safe-area root *R* (800 × 600, design left −400, so 432 at C8) holding a Type A
/// container *X* (800 × 400 at (0, 100)) with one 200 × 100 child at (200, 150).
fn build_10_8_b() -> Ui {
    let kid = edges(field(K108, 200, 150, 200, 100), [0, 0, 0, 0]);
    let x = child(
        stretch(edges(field(X108, 0, 100, 800, 400), [6, 5, 6, 5])),
        kid,
    );
    let r = child(edges(field(R108, -400, 100, 800, 600), [0, 0, 0, 0]), x);
    let env = env_with(vec![layout(L108, 1024, 768, vec![r])]);
    ui_with(env, 1024, 768)
}

fn open_10_8_b(ui: &mut Ui) -> [tessera_ui::ElementKey; 3] {
    let r = ui.open_panel(Did(L108), R108).unwrap();
    let x = ui.find_child(r, X108).unwrap();
    let k = ui.find_child(x, K108).unwrap();
    [r, x, k]
}

/// Asserts the §10.8 (b) outcome at C8 with *m*(*R*) = 2: *X* displayed at 232 (fitted
/// position −100 × 2 from *R*'s 432), 2400 × 1200 (the real-valued displayed box of §3.4; the
/// truncating rule of UB §9.2.1 gave 2402, `ui-scaling.md` §5.9 answer), *f* = 1.5, and its
/// child at 832, the safe area's left end.
fn assert_10_8_b(ui: &Ui, [r, x, k]: [tessera_ui::ElementKey; 3], what: &str) {
    let rd = ui.displayed_box(r);
    assert_eq!((rd.l, rd.t), (432, 100), "{what}: R displayed top-left");
    near(f64::from(ui.fit_factor(x)), 1.5, 1e-6, "f");
    let xd = ui.displayed_box(x);
    assert_eq!(xd.l, 232, "{what}: X displayed left (§10.8 (b) step 4)");
    assert_eq!(xd.t, 100, "{what}: X displayed top");
    assert_eq!(xd.w(), 2400, "{what}: X width");
    assert_eq!(xd.h(), 1200, "{what}: X height");
    assert_eq!(
        ui.node_scale_at_or_above(x).0,
        3.0,
        "{what}: m(X) = f × m(R)"
    );
    assert_eq!(
        ui.displayed_box(k).l,
        832,
        "{what}: child at the safe left end"
    );
}

#[test]
fn example_10_8_b_fit_under_a_node_scale() {
    let mut ui = build_10_8_b();
    let keys = open_10_8_b(&mut ui);
    ui.set_node_scale(keys[0], Some([2.0, 2.0, 1.0]));
    apply(&mut ui, &C8);
    assert_10_8_b(&ui, keys, "scale before C8");
}

/// A per-window scale set after the layout fits the containers inside again (no stale fit
/// until the next screen change), and so does a move of the root: the child stays at the safe
/// area's left end when *R* is moved right by 100 (fitted position −150, displayed 532 − 300).
#[test]
fn fit_follows_scale_change_and_move_of_its_root() {
    let mut ui = build_10_8_b();
    let keys = open_10_8_b(&mut ui);
    let [r, x, k] = keys;
    apply(&mut ui, &C8);
    assert_eq!(ui.displayed_box(x).l, 432, "m = 1: X at R's left");
    ui.set_node_scale(r, Some([2.0, 2.0, 1.0]));
    assert_10_8_b(&ui, keys, "scale after C8");
    let b = ui.layout_box(r).unwrap();
    ui.move_to(r, b.l + 100, b.t);
    assert_eq!(ui.displayed_box(r).l, 532);
    assert_eq!(ui.displayed_box(x).l, 232, "refitted after the move");
    assert_eq!(
        ui.displayed_box(k).l,
        832,
        "child still at the safe left end"
    );
    // Back to no scale: refitted with m = 1 again.
    ui.set_node_scale(r, None);
    assert_eq!(ui.node_scale_at_or_above(x).0, ui.fit_factor(x));
}

// ----- review regressions: modal dialogs and fit-container roots in the safe area (§5.2) -----

const LDLG: u32 = 0x22FF_5900;
const DLG: u32 = 0x1000_5901;
const BIG: u32 = 0x1000_5902;
const BROOT: u32 = 0x1000_5903;
const BROOT0: u32 = 0x1000_5904;

fn modal(d: tessera_formats::ui_layout::ElementDesc) -> tessera_formats::ui_layout::ElementDesc {
    prop(d, v().pids.dialog_modal, b(true))
}

fn build_dialogs(is_modal: bool) -> Ui {
    let m = |d| if is_modal { modal(d) } else { d };
    let dlg = m(edges(field(DLG, 800, 10, 200, 100), [4, 5, 5, 0]));
    let big = m(edges(field(BIG, 0, 0, 1300, 900), [1, 1, 1, 1]));
    let env = env_with(vec![layout(LDLG, 1024, 768, vec![dlg, big])]);
    ui_with(env, 1024, 768)
}

/// §5.2: a modal dialog is laid out against its container's safe box, so at C8 it ends where
/// the same panel opened non-modal ends: at the safe edge 4288 − 24.
#[test]
fn modal_dialog_in_safe_box_c8() {
    for is_modal in [false, true] {
        let mut ui = build_dialogs(is_modal);
        let d = ui.open_panel(Did(LDLG), DLG).unwrap();
        assert_eq!(
            ui.parent(d) != Some(ui.screen_root()),
            is_modal,
            "modal container"
        );
        apply(&mut ui, &C8);
        assert_eq!(
            ui.displayed_box(d).r + 1,
            4264,
            "modal {is_modal}: right end"
        );
        apply(&mut ui, &C9);
        assert_eq!(
            ui.displayed_box(d).r + 1,
            5096,
            "modal {is_modal}: C9 right end"
        );
        apply(&mut ui, &C8);
        assert_eq!(
            ui.displayed_box(d).r + 1,
            4264,
            "modal {is_modal}: back at C8"
        );
    }
}

/// §5.6 and keep-on-screen for a modal dialog larger than the screen (C1): it gets *q* and is
/// kept inside *F*, like a non-modal root.
#[test]
fn modal_dialog_fit_to_screen_c1() {
    let mut ui = build_dialogs(true);
    let d = ui.open_panel(Did(LDLG), BIG).unwrap();
    apply(&mut ui, &C1);
    near(
        f64::from(ui.fit_to_screen_factor(d)),
        768.0 / 900.0,
        1e-4,
        "dialog q",
    );
    let b = ui.displayed_box(d);
    assert!(
        b.l >= 0 && b.t >= 0 && b.r < 1365 && b.b < 768,
        "dialog kept on screen: {b:?}"
    );
}

/// §5.2 with §5.4 Type B: a Stretch root (5, 5, 6, 5) that is not full-screen is a safe-area
/// panel, so at C8 it starts at A's left (832), like the same root with edge mode 0 instead of
/// 6; f = min(1440 ÷ 900, 3456 ÷ 500) = 1.6.
#[test]
fn type_b_root_in_safe_area_c8() {
    let b6 = stretch(edges(field(BROOT, 0, 0, 500, 900), [5, 5, 6, 5]));
    let b0 = stretch(edges(field(BROOT0, 0, 0, 500, 900), [5, 5, 0, 5]));
    let env = env_with(vec![layout(LDLG, 1440, 900, vec![b6, b0])]);
    let mut ui = ui_with(env, 1024, 768);
    let k6 = ui.open_panel(Did(LDLG), BROOT).unwrap();
    let k0 = ui.open_panel(Did(LDLG), BROOT0).unwrap();
    apply(&mut ui, &C8);
    assert_eq!(ui.displayed_box(k0).l, 832, "mode-0 root at A's left");
    assert_eq!(ui.displayed_box(k6).l, 832, "Type B root at A's left");
    near(f64::from(ui.fit_factor(k6)), 1.6, 1e-6, "Type B f");
    apply(&mut ui, &C9);
    assert_eq!(ui.displayed_box(k6).l, 0, "C9: A = F");
}

// ----- review regressions: fit-group cost and the member cap (§5.4) -------------------------

const LBIG: u32 = 0x22FF_5A00;
const BIGROOT: u32 = 0x1000_5A01;

/// One (5, 5, 5, 5) root holding `n` wrappers of distinct widths, each wrapping one Type A
/// container (so no two group).
fn build_wrapped(n: u32) -> (Ui, tessera_ui::ElementKey) {
    let mut root = edges(field(BIGROOT, 0, 0, 1024, 768), [5, 5, 5, 5]);
    for i in 0..n {
        let w = 10 + i as i32;
        let wrap = child(
            edges(field(0x2000_0000 + i, 0, 0, w, 50), [0, 0, 0, 0]),
            stretch(edges(field(0x3000_0000 + i, 0, 0, w, 50), [6, 5, 6, 5])),
        );
        root = child(root, wrap);
    }
    let env = env_with(vec![layout(LBIG, 1024, 768, vec![root])]);
    let mut ui = ui_with(env, 1024, 768);
    let r = ui.open_panel(Did(LBIG), BIGROOT).unwrap();
    (ui, r)
}

/// Many groups of one in one root panel: every screen change fits each of them once from one
/// partition of the root, not with a search per member (the review measured seconds at 2,000
/// before). The timing bound is asserted only with `TESSERA_PERF=1` (`docs/DEVELOPMENT.md`).
#[test]
fn many_fit_containers_lay_out_linearly() {
    let (mut ui, r) = build_wrapped(2000);
    let t = std::time::Instant::now();
    apply(&mut ui, &C2);
    apply(&mut ui, &C8);
    apply(&mut ui, &C1);
    let took = t.elapsed();
    // Each container is fitted alone: f = min(cover, cap) of its own 50-high box in a 50-high P.
    let wrap = ui.find_child(r, 0x2000_0000 + 1999).unwrap();
    let x = ui.find_child(wrap, 0x3000_0000 + 1999).unwrap();
    near(
        f64::from(ui.fit_factor(x)),
        1.0,
        1e-6,
        "group of one in a same-size P",
    );
    if std::env::var_os("TESSERA_PERF").is_some() {
        assert!(took.as_millis() < 500, "three screen changes took {took:?}");
    }
}

/// More than [`MAX_GROUP_MEMBERS`] (64) containers of one group: the cut is the same whichever
/// member is laid out, so the first 64 in drawing order share one factor (and keep it across
/// screen changes), and the rest are groups of one. Their bars are one group's, reported once.
#[test]
fn group_member_cap_is_deterministic() {
    let n = 100;
    let mut root = edges(field(BIGROOT, 0, 0, 1024, 768), [5, 5, 5, 5]);
    for i in 0..n {
        let (x, w) = if i < 64 {
            (0, 1024)
        } else {
            (-(i * 10), 1024 + i * 20)
        };
        let ta = child(
            stretch(edges(
                field(0x3000_0000 + i as u32, x, 0, w, 768),
                [6, 5, 6, 5],
            )),
            edges(field(0x4000_0000 + i as u32, 0, 0, w, 768), [0, 0, 0, 0]),
        );
        root = child(root, ta);
    }
    let env = env_with(vec![layout(LBIG, 1024, 768, vec![root])]);
    let mut ui = ui_with(env, 1024, 768);
    let r = ui.open_panel(Did(LBIG), BIGROOT).unwrap();
    let keys: Vec<_> = (0..n)
        .map(|i| ui.find_child(r, 0x3000_0000 + i as u32).unwrap())
        .collect();
    let factors = |ui: &Ui| keys.iter().map(|k| ui.fit_factor(*k)).collect::<Vec<_>>();
    apply(&mut ui, &C2);
    let at_c2 = factors(&ui);
    let head = at_c2[0];
    assert!(
        at_c2[..64].iter().all(|f| *f == head),
        "one factor for the capped group: {:?}",
        &at_c2[..64]
    );
    near(f64::from(head), 1.40625, 1e-4, "the head group's factor");
    apply(&mut ui, &C8);
    apply(&mut ui, &C2);
    assert_eq!(factors(&ui), at_c2, "the same cut after a round trip");
    assert!(
        ui.letterbox_bars().len() <= 1,
        "one group's bars at most once"
    );
}

/// Many fit roots of one layout are one group for the letterbox bars: one set of bars, below the
/// lowest shown root, and a pointer in a bar hits that root.
#[test]
fn many_fit_roots_one_bar_set() {
    let n = 300u32;
    let els: Vec<_> = (0..n)
        .map(|i| {
            child(
                edges(field(0x1000_0000 + i, 0, 0, 1024, 768), [6, 5, 6, 5]),
                edges(field(0x5000_0000 + i, 0, 0, 1024, 768), [0, 0, 0, 0]),
            )
        })
        .collect();
    let env = env_with(vec![layout(LBIG, 1024, 768, els)]);
    let mut ui = ui_with(env, 1024, 768);
    let roots: Vec<_> = (0..n)
        .map(|i| ui.open_panel(Did(LBIG), 0x1000_0000 + i).unwrap())
        .collect();
    apply(&mut ui, &C2);
    let bars = ui.letterbox_bars();
    assert_eq!(bars.len(), 1, "one set of bars");
    assert_eq!(bars[0].below, roots[0], "below the lowest shown root");
    assert_eq!(
        ui.hit_test(50, 500),
        Some(roots[0]),
        "bar hits the lowest root"
    );
}

// ----- review regressions: saved placements applied again (§4.5 step 2, §5.8) ----------------

const LSAVE: u32 = 0x22FF_5B00;
const SAVED: u32 = 0x1000_5B01;
const SAVEBAR: u32 = 0x1000_5B02;

fn build_saved(pid: u32) -> Ui {
    let bar = typed(SAVEBAR, v().types.dragbar, 0, 0, 200, 20);
    let root = child(
        prop(
            edges(field(SAVED, 100, 40, 200, 100), [0, 0, 0, 0]),
            pid,
            b(true),
        ),
        z(bar, 0, 1),
    );
    let env = env_with(vec![layout(LSAVE, 1024, 768, vec![root])]);
    ui_with(env, 1024, 768)
}

/// A user drag of a `save_loc` panel records its placement (§5.8 "Saving"), so a placement
/// applied again after a change of *A* or *F* keeps the dragged spot, not the start-up one.
#[test]
fn dragged_saved_panel_keeps_its_place() {
    let mut ui = build_saved(v().pids.save_loc);
    let k = ui.open_panel(Did(LSAVE), SAVED).unwrap();
    apply(&mut ui, &C8);
    ui.move_to(k, 1000, 40);
    ui.record_saved(k); // the start-up placement
    // Drag by the bar from (1010, 45) to (2010, 305): to (2000, 300).
    ui.pointer_moved(1010, 45);
    ui.mouse_press(&cmd("SelectLeft"));
    ui.pointer_moved(2010, 305);
    ui.mouse_release(&cmd("SelectLeft"));
    assert_eq!(ui.displayed_box(k).l, 2000, "dragged");
    apply(&mut ui, &C9); // A changes, F does not
    let d = ui.displayed_box(k);
    assert_eq!(
        (d.l, d.t),
        (2000, 300),
        "A-only change keeps the dragged spot"
    );
    apply(&mut ui, &C8);
    assert_eq!(ui.displayed_box(k).l, 2000, "back at C8");
    // An F change places it by the anchor saved at the drag (centred, 460 left of A's centre).
    apply(&mut ui, &C2);
    assert_eq!(
        ui.displayed_box(k).l,
        960 - 460 - 100,
        "anchored from the drag"
    );
}

/// The visibility a placement carries is applied when the panel is opened, not again after a
/// window resize: a hidden `save_visibility` panel stays hidden.
#[test]
fn hidden_saved_panel_stays_hidden_on_resize() {
    let mut ui = build_saved(v().pids.save_visibility);
    let k = ui.open_panel(Did(LSAVE), SAVED).unwrap();
    apply(&mut ui, &C2);
    ui.record_saved(k); // saved while shown
    ui.set_shown(k, false);
    apply(&mut ui, &C1);
    assert!(!ui.is_visible(k), "still hidden after a resize");
    assert!(ui.apply_saved(k), "applied when opened");
    assert!(ui.is_visible(k), "the open-time application shows it");
}

// ----- §7.4: the per-window scale --------------------------------------------------------------

/// §7.4: a per-window factor is a uniform node scale. The layout box keeps its size, the
/// children keep their layout boxes, the scaling origin stays in place in UI units, the
/// displayed box is the layout box × *p* (§3.4), and §5.6 / keep-on-screen run after it.
#[test]
fn per_window_scale_is_a_uniform_node_scale() {
    let kid = edges(field(CHILD_A, 20, 30, 100, 40), [0, 0, 0, 0]);
    let win = child(
        prop(
            prop(
                prop(
                    edges(field(ROOT, 600, 100, 400, 300), [0, 0, 0, 0]),
                    v().pids.scaler_group,
                    int(7),
                ),
                v().pids.scaler_origin_x,
                int(400),
            ),
            v().pids.scaler_origin_y,
            int(0),
        ),
        kid,
    );
    let env = env_with(vec![layout(LAYOUT, 1024, 768, vec![win])]);
    let mut ui = ui_with(env, 1024, 768);
    let k = ui.open_panel(Did(LAYOUT), ROOT).unwrap();
    let c = ui.find_child(k, CHILD_A).unwrap();
    apply(&mut ui, &C2);
    let before = ui.displayed_box(k);
    assert_eq!(ui.window_scale(k), 1.0);
    assert!(ui.set_window_scale(k, 1.5));
    assert_eq!(ui.window_scale(k), 1.5);
    assert_eq!(ui.layout_box(k).unwrap().w(), 400, "layout size kept");
    assert_eq!(
        ui.layout_box(c).unwrap().w(),
        100,
        "children not laid out again"
    );
    let d = ui.displayed_box(k);
    assert_eq!((d.w(), d.h()), (600, 450), "displayed 400 × 300 × 1.5");
    // The origin (400, 0) is the panel's top-right corner: it stays where it was.
    assert_eq!(d.r, before.r, "the origin's x stays in place");
    assert_eq!(d.t, before.t, "the origin's y stays in place");
    let cd = ui.displayed_box(c);
    assert_eq!(
        (cd.l - d.l, cd.t - d.t, cd.w()),
        (30, 45, 150),
        "child scales with it"
    );
    // Back to 1: the original box again.
    assert!(ui.set_window_scale(k, 1.0));
    assert_eq!(ui.displayed_box(k), before);
    assert_eq!(ui.node_scale_at_or_above(k).0, 1.0);
    // §5.6 multiplies p on a screen it does not fit (C1: 768 units high, 300 × 3 = 900).
    assert!(ui.set_window_scale(k, 3.0));
    apply(&mut ui, &C1);
    near(
        f64::from(ui.fit_to_screen_factor(k)),
        768.0 / 900.0,
        1e-4,
        "q over p",
    );
    assert_eq!(ui.window_scale(k), 3.0, "p is kept apart from q");
    let d = ui.displayed_box(k);
    assert!(
        d.l >= 0 && d.t >= 0 && d.r < 1365 && d.b < 768,
        "kept on screen: {d:?}"
    );
    // Not a window scale target: bad factors and elements that are no root panel.
    assert!(!ui.set_window_scale(k, 0.0));
    assert!(!ui.set_window_scale(k, f32::NAN));
    assert!(!ui.set_window_scale(c, 1.5));
    assert_eq!(ui.screen_root_of(c), Some(k));
    assert_eq!(ui.screen_root_of(k), Some(k));
}

// ----- §3.4 / §3.6: physical boxes and hits from the real displayed boxes (review) -----------

const C3: Config = Config {
    name: "C3",
    wp: 1920,
    hp: 1080,
    s: 1.25,
    aspect: A21,
    wu: 1536,
    hu: 864,
};
const C11: Config = Config {
    name: "C11",
    wp: 7680,
    hp: 4320,
    s: 3.0,
    aspect: A21,
    wu: 2560,
    hu: 1440,
};

/// §3.6 "Box → physical" on the real box: `Ex(L) … Ex(R) − 1`, `Ey(T) … Ey(B) − 1`.
fn physical_of_real(f: &UiScreen, d: &DBox) -> Rect {
    Rect::new(
        f.edge_x(d.l),
        f.edge_y(d.t),
        f.edge_x(d.l + d.w),
        f.edge_y(d.t + d.h),
    )
}

/// §3.4, §3.6: at C1 the theatre group (f = 1, §10.6) is centred at a real left of 170.5, so
/// its progress child starts at 581.5 UI units. The physical boxes map those real edges, not
/// edges first rounded to whole units (581.5 would round to 582 and start one column late), and
/// the hit test at a pixel centre agrees with them exactly (I3).
#[test]
fn physical_boxes_and_hits_use_the_real_edges_c1() {
    let mut ui = build_theatre();
    let [s, _, k] = open_theatre(&mut ui);
    ui.set_property(k, v().pids.blocks_clicks, b(true));
    let f = apply(&mut ui, &C1);
    let sd = ui.displayed_real(s);
    near(sd.l, 170.5, 1e-9, "story real left");
    let kd = ui.displayed_real(k);
    near(kd.l, 581.5, 1e-9, "progress child real left");
    for e in [s, k] {
        assert_eq!(
            ui.physical_box(e),
            physical_of_real(&f, &ui.displayed_real(e)),
            "physical box from the real box"
        );
    }
    let x0 = ui.physical_box(k).x0;
    assert_eq!(x0, f.edge_x(581.5));
    assert_ne!(x0, f.edge_x(582.0), "not the rounded edge");
    // I3: the pixel centres on either side of the edge, through the pointer path.
    let row = ui.physical_box(k).y0 + 2;
    let at = |ui: &mut Ui, col: i32| {
        ui.offer(&tessera_ui::UiInput::PointerMoved {
            x: f.pointer_x(col),
            y: f.pointer_y(row),
        });
        ui.hover_refresh();
        ui.hit_element()
    };
    let inside = at(&mut ui, x0);
    let outside = at(&mut ui, x0 - 1);
    assert_eq!(inside, Some(k), "the edge column hits the child");
    assert_ne!(outside, Some(k), "the column before it does not");
    assert_eq!(
        ui.hit_test_real(f.pointer_x(x0), f.pointer_y(row)),
        inside,
        "hit_test_real agrees with the pointer path"
    );
}

/// §3.4 / §3.6 at C3 (K = 1.25) and C11 (K = 3) with a per-window factor of 1.05, whose
/// children and inner fit container have fractional edges: every physical box is the §3.6
/// mapping of the real displayed box, and hits at pixel centres agree with the physical boxes.
#[test]
fn physical_boxes_follow_fractional_edges_c3_c11() {
    for c in [&C3, &C11] {
        let kid = prop(
            edges(field(CHILD_A, 21, 33, 101, 41), [0, 0, 0, 0]),
            v().pids.blocks_clicks,
            b(true),
        );
        let inner = child(
            edges(field(CHILD_B, 0, 0, 300, 150), [6, 5, 6, 5]),
            edges(field(0x1000_5106, 0, 0, 300, 150), [0, 0, 0, 0]),
        );
        let win = child(
            child(
                prop(
                    edges(field(ROOT, 100, 100, 400, 300), [0, 0, 0, 0]),
                    v().pids.scaler_group,
                    int(7),
                ),
                kid,
            ),
            child(
                edges(field(0x1000_5105, 10, 90, 380, 200), [0, 0, 0, 0]),
                inner,
            ),
        );
        let env = env_with(vec![layout(LAYOUT, 1024, 768, vec![win])]);
        let mut ui = ui_with(env, 1024, 768);
        let k = ui.open_panel(Did(LAYOUT), ROOT).unwrap();
        let a = ui.find_child(k, CHILD_A).unwrap();
        let f = apply(&mut ui, c);
        assert!(ui.set_window_scale(k, 1.05));
        let holder = ui.find_child(k, 0x1000_5105).unwrap();
        let inner = ui.find_child(holder, CHILD_B).unwrap();
        let mut fractional = 0;
        for e in [k, a, holder, inner] {
            let d = ui.displayed_real(e);
            if d.l.fract() != 0.0 || (d.l + d.w).fract() != 0.0 {
                fractional += 1;
            }
            let p = ui.physical_box(e);
            assert_eq!(p, physical_of_real(&f, &d), "{} physical box", c.name);
        }
        assert!(fractional > 0, "{}: some edge is fractional", c.name);
        // §3.4 item 2: the inner fit container is fitted again under p (m = f × p).
        let fi = f64::from(ui.fit_factor(inner));
        near(
            f64::from(ui.node_scale_at_or_above(inner).0),
            fi * 1.05,
            1e-4,
            "m(inner) = f × p",
        );
        let id = ui.displayed_real(inner);
        let hd = ui.displayed_real(holder);
        near(id.w, 300.0 * fi * 1.05, 1e-3, "inner displayed width");
        assert!(
            id.l >= hd.l - 1e-9 && id.l + id.w <= hd.l + hd.w + 1e-6 || id.w >= hd.w - 1e-6,
            "{}: inner within or covering its parent",
            c.name
        );
        // The child's physical box holds exactly the pixel centres that hit it (I3).
        let p = ui.physical_box(a);
        let row = (p.y0 + p.y1) / 2;
        for col in [p.x0 - 1, p.x0, p.x1 - 1, p.x1] {
            let ux = f.pointer_x(col);
            let d = ui.displayed_real(a);
            let in_real = ux >= d.l && ux < d.l + d.w;
            let in_phys = col >= p.x0 && col < p.x1;
            assert_eq!(in_real, in_phys, "{} column {col}", c.name);
            let hit = ui.hit_test_real(ux, f.pointer_y(row)) == Some(a);
            assert_eq!(hit, in_phys, "{} hit at column {col}", c.name);
        }
    }
}

/// §7.4 origin rule without drift: with origin x = 30 and a left of 300, twenty steps of 0.05
/// up and twenty down return the panel to 300, and at p = 2 the origin is still at 330.
#[test]
fn per_window_scale_round_trip_does_not_creep() {
    let win = prop(
        prop(
            edges(field(ROOT, 300, 200, 200, 100), [0, 0, 0, 0]),
            v().pids.scaler_group,
            int(7),
        ),
        v().pids.scaler_origin_x,
        int(30),
    );
    let env = env_with(vec![layout(LAYOUT, 1024, 768, vec![win])]);
    let mut ui = ui_with(env, 1024, 768);
    let k = ui.open_panel(Did(LAYOUT), ROOT).unwrap();
    apply(&mut ui, &C2);
    let start = ui.layout_box(k).unwrap();
    assert_eq!(start.l, 300);
    for cycle in 0..3 {
        for i in 1..=20 {
            assert!(ui.set_window_scale(k, 1.0 + 0.05 * i as f32));
        }
        let l = ui.layout_box(k).unwrap().l;
        assert_eq!(l, 270, "cycle {cycle}: origin 330 − 30 × 2");
        for i in (0..20).rev() {
            assert!(ui.set_window_scale(k, 1.0 + 0.05 * i as f32));
        }
        assert_eq!(ui.layout_box(k).unwrap(), start, "cycle {cycle}: back home");
    }
}

/// §3.4: a per-window factor multiplies a Stretch root's stretch vector, in both orders, and a
/// panel scaled by the panel-scale rule takes a per-window factor again once it is back at 1.
#[test]
fn per_window_scale_composes_with_stretch_and_panel_scale() {
    let win = prop(
        stretch(edges(field(ROOT, 100, 100, 200, 100), [0, 0, 0, 0])),
        v().pids.scaler_group,
        int(29),
    );
    let env = env_with(vec![layout(LAYOUT, 1024, 768, vec![win])]);
    let mut ui = ui_with(env, 1024, 768);
    let k = ui.open_panel(Did(LAYOUT), ROOT).unwrap();
    apply(&mut ui, &C2);
    // Stretch first (300 × 100 shown for a 200 × 100 box), then p = 1.5.
    ui.resize(k, 300, 100);
    near(
        f64::from(ui.node_scale_at_or_above(k).0),
        1.5,
        1e-6,
        "stretch",
    );
    assert!(ui.set_window_scale(k, 1.5));
    let s = ui.node_scale_at_or_above(k);
    near(f64::from(s.0), 2.25, 1e-5, "stretch × p, x");
    near(f64::from(s.1), 1.5, 1e-5, "stretch × p, y");
    // A user resize changes only the stretch; p stays applied.
    ui.resize(k, 200, 200);
    let s = ui.node_scale_at_or_above(k);
    near(f64::from(s.0), 1.5, 1e-5, "new stretch 1 × p");
    near(f64::from(s.1), 3.0, 1e-5, "new stretch 2 × p");
    assert_eq!(ui.window_scale(k), 1.5);
    // Back to p = 1: the stretch alone remains.
    assert!(ui.set_window_scale(k, 1.0));
    let s = ui.node_scale_at_or_above(k);
    near(f64::from(s.0), 1.0, 1e-5, "stretch x");
    near(f64::from(s.1), 2.0, 1e-5, "stretch y");

    // The panel-scale rule owns the node scale while it is not 1.
    let plain = edges(field(ROOT, 100, 100, 200, 100), [0, 0, 0, 0]);
    let env = env_with(vec![layout(LAYOUT, 1024, 768, vec![plain])]);
    let mut ui = ui_with(env, 1024, 768);
    let k = ui.open_panel(Did(LAYOUT), ROOT).unwrap();
    apply(&mut ui, &C2);
    ui.scale_panel(k, 1.5, true);
    assert!(!ui.accepts_window_scale(k));
    assert!(!ui.set_window_scale(k, 1.25));
    ui.scale_panel(k, 1.0, true);
    assert!(
        ui.accepts_window_scale(k),
        "back at 1 the panel is free again"
    );
    assert!(ui.set_window_scale(k, 1.25));
    assert_eq!(ui.window_scale(k), 1.25);
}

/// §3.4 item 2 under a per-window factor: the §10.8 (b) fit container inside a root with
/// p = 2 (set through §7.4, not a raw node scale) has the §10.8 (b) displayed box.
#[test]
fn fit_container_under_a_per_window_factor_10_8_b() {
    let mut ui = build_10_8_b();
    let keys = open_10_8_b(&mut ui);
    apply(&mut ui, &C8);
    assert!(ui.set_window_scale(keys[0], 2.0));
    assert_10_8_b(&ui, keys, "p = 2 after C8");
}

/// §4.5 step 3 on the tessera-ui side (the part the G7 render golden cannot see, since
/// `tessera-render` does not depend on `tessera-ui`): in the frames right after C2 → C3, while
/// the new paint generation is still being painted band by band, the composition list composes
/// the panel's **old** canvas (painted at C2) onto its **new** §3.6 physical box, never a blank
/// or a stale destination (I9); once the generation is swapped in, the canvas is the new
/// physical size, 1 : 1 on that box.
#[test]
fn scale_change_previews_the_old_canvas_on_the_new_box() {
    let win = media(
        edges(field(ROOT, 760, 390, 400, 300), [0, 0, 0, 0]),
        image_item(0x4100_5101, 2),
    );
    let mut env = env_with(vec![layout(LAYOUT, 1920, 1080, vec![win])]);
    add_image(&mut env, 0x4100_5101, solid(8, 8, 0xFF20_3040));
    let mut ui = ui_with(env, 1920, 1080);
    let k = ui.open_panel(Did(LAYOUT), ROOT).unwrap();
    apply(&mut ui, &C2);
    let mut t = 1u64;
    let mut fr = UiFrame::default();
    for _ in 0..6 {
        t += 16;
        fr = ui.paint(Duration::from_millis(t), &mut Unlimited);
    }
    let old = fr.composition.entries.last().cloned().expect("panel entry");
    assert_eq!(old.canvas, [400, 300], "painted at c = 1");
    assert_eq!(old.dest, ui.physical_box(k));
    apply(&mut ui, &C3);
    let new_box = ui.physical_box(k);
    assert_eq!(
        (new_box.width(), new_box.height()),
        (500, 375),
        "C3 physical size"
    );
    // One band per frame: the 375-row canvas takes many frames to repaint.
    let mut previews = 0;
    let mut settled = false;
    for _ in 0..200 {
        t += 16;
        let fr = ui.paint(Duration::from_millis(t), &mut BandBudget(1));
        let e = fr.composition.entries.last().expect("always composed (I9)");
        assert_eq!(e.dest, new_box, "the new physical box from the first frame");
        if e.canvas == [400, 300] {
            assert!(!settled, "no return to the old picture");
            previews += 1;
        } else {
            assert_eq!(e.canvas, [500, 375], "repainted 1 : 1 on the new box");
            settled = true;
        }
    }
    assert!(
        previews > 0,
        "the old picture is shown while the new one is painted"
    );
    assert!(settled, "the new generation is swapped in");
}
