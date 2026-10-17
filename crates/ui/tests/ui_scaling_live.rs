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
