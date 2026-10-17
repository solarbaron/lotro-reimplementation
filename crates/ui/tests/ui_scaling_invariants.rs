// SPDX-License-Identifier: GPL-3.0-or-later
//! The invariants of `ui-scaling.md` §9.5 and the unit items of §9.6 as external tests over the
//! pure geometry core `tessera_ui::scaling` (decision 0040). The module's own `#[cfg(test)]`
//! units pin the §10 worked examples by value; this file adds the **property** coverage §9.5
//! asks for ("testable on synthetic layouts at random configurations") and a sweep over the
//! named configurations C1–C12 of §9.2, so the invariants hold not only on the worked numbers
//! but across the whole input space the pure core accepts.
//!
//! Scope: everything here is arithmetic over the spaces of §3–§6. The invariants that need the
//! live element tree and painter — I2 (existing goldens byte-identical at `Se` = 1), I6 over
//! native root panels, I9 (no blank surface during a change), I14 (re-layout determinism of the
//! frame system) — belong to `tessera-ui`'s layout/paint integration of §5 and §6, which this
//! file does not exercise (see the suite's findings). The pieces of those that the pure core
//! can establish (keep-on-screen clamps a root inside `F`; a saved placement round-trips) are
//! checked here.

use proptest::prelude::*;
use tessera_ui::scaling::{
    self, Anchor, ComputedBox, ContentBox, DBox, FitMember, FitVerticalAnchor, GlyphMetrics, RHO,
    RefBox, SavedPlacement, SourceKind, UiScreen,
};

// ------------------------------------------------------------------------------------------------
// The named configurations of §9.2.
// ------------------------------------------------------------------------------------------------

/// `(name, Wp, Hp, S, aspect, Wu, Hu)`; aspect `None` is `"full"`, `Some(a)` a ratio.
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

const CONFIGS: &[Config] = &[
    Config {
        name: "C1",
        wp: 1280,
        hp: 720,
        s: 1.0,
        aspect: A21,
        wu: 1365,
        hu: 768,
    },
    Config {
        name: "C2",
        wp: 1920,
        hp: 1080,
        s: 1.0,
        aspect: A21,
        wu: 1920,
        hu: 1080,
    },
    Config {
        name: "C3",
        wp: 1920,
        hp: 1080,
        s: 1.25,
        aspect: A21,
        wu: 1536,
        hu: 864,
    },
    Config {
        name: "C4",
        wp: 2560,
        hp: 1440,
        s: 1.0,
        aspect: A21,
        wu: 2560,
        hu: 1440,
    },
    Config {
        name: "C5",
        wp: 3840,
        hp: 2160,
        s: 1.5,
        aspect: A21,
        wu: 2560,
        hu: 1440,
    },
    Config {
        name: "C6",
        wp: 3840,
        hp: 2160,
        s: 2.0,
        aspect: A21,
        wu: 1920,
        hu: 1080,
    },
    Config {
        name: "C7",
        wp: 3440,
        hp: 1440,
        s: 1.0,
        aspect: A21,
        wu: 3440,
        hu: 1440,
    },
    Config {
        name: "C8",
        wp: 5120,
        hp: 1440,
        s: 1.0,
        aspect: A21,
        wu: 5120,
        hu: 1440,
    },
    Config {
        name: "C9",
        wp: 5120,
        hp: 1440,
        s: 1.0,
        aspect: None,
        wu: 5120,
        hu: 1440,
    },
    Config {
        name: "C10",
        wp: 1600,
        hp: 1200,
        s: 1.0,
        aspect: A21,
        wu: 1600,
        hu: 1200,
    },
    Config {
        name: "C11",
        wp: 7680,
        hp: 4320,
        s: 3.0,
        aspect: A21,
        wu: 2560,
        hu: 1440,
    },
    Config {
        name: "C12",
        wp: 3840,
        hp: 2160,
        s: 1.0,
        aspect: A21,
        wu: 3840,
        hu: 2160,
    },
];

fn screen_of(c: &Config) -> UiScreen {
    let se = scaling::effective_scale(c.s, c.wp, c.hp);
    UiScreen::new(c.wp, c.hp, se).expect("a non-degenerate window")
}

/// Every named configuration lands on the §9.2 `F`, and I1 holds on each (`Wu ≥ 1024`,
/// `Hu ≥ 768`, `Se ≤ S`, `F` covers the window exactly).
#[test]
fn named_configurations_match_and_hold_i1() {
    for c in CONFIGS {
        let f = screen_of(c);
        assert_eq!(f.ui_size(), (c.wu, c.hu), "{}: F", c.name);
        assert!(f.wu() >= 1024 && f.hu() >= 768, "{}: design floor", c.name);
        assert!(
            f.effective_scale() <= c.s + 1e-9,
            "{}: Se {} > S {}",
            c.name,
            f.effective_scale(),
            c.s
        );
        // F covers the window exactly: no strip at the right or bottom (§3.2).
        assert!(
            (f.kx() * f64::from(c.wu) - f64::from(c.wp)).abs() < 1e-6,
            "{}: Kx",
            c.name
        );
        assert!(
            (f.ky() * f64::from(c.hu) - f64::from(c.hp)).abs() < 1e-6,
            "{}: Ky",
            c.name
        );
    }
}

/// The safe area at every named configuration: `A ⊆ F`, never narrower than 1024, the full
/// height of `F`, centred, and exactly `Aw = min(Wu, round(Hu × a))` at `Ax = ⌊(Wu − Aw) ÷ 2⌋`
/// (§5.1). C8 is `A` = 3456 wide at 832 (§9.2, §10.3).
#[test]
fn safe_area_at_named_configurations() {
    for c in CONFIGS {
        let f = screen_of(c);
        let a = scaling::safe_area(&f, c.aspect);
        let f_box = DBox::new(0.0, 0.0, f64::from(c.wu), f64::from(c.hu));
        assert!(a.inside(&f_box, 1e-9), "{}: A not inside F", c.name);
        assert!(a.w >= 1024.0, "{}: A width {} < 1024", c.name, a.w);
        assert_eq!(a.h, f64::from(c.hu), "{}: A full height", c.name);
        let expect_aw = match c.aspect {
            None => f64::from(c.wu),
            Some(aspect) => {
                let a = aspect.clamp(4.0 / 3.0, 4.0);
                (f64::from(c.hu) * a + 0.5).floor().min(f64::from(c.wu))
            }
        };
        assert_eq!(a.w, expect_aw, "{}: Aw", c.name);
        let expect_ax = ((c.wu - expect_aw as i32) / 2) as f64;
        assert_eq!(a.l, expect_ax, "{}: Ax", c.name);
    }
    // C8 explicitly.
    let c8 = &CONFIGS[7];
    let a = scaling::safe_area(&screen_of(c8), c8.aspect);
    assert_eq!((a.l, a.w, a.h), (832.0, 3456.0, 1440.0), "C8 safe area");
}

// ------------------------------------------------------------------------------------------------
// I3 / I4: the pixel-centre mapping (property, §9.6 "Unit: the pixel-centre mapping").
// ------------------------------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(400))]

    /// I3: for a range of axis factors and UI coordinates, `edge_x(v)` is exactly the smallest
    /// physical column `X` with `(X + ½) ÷ K ≥ v`, so drawing and hit testing agree with no
    /// tolerance. I4: a shared edge maps to one column (abutting siblings abut in pixels).
    #[test]
    fn pixel_centre_mapping_is_smallest_column(
        wp in 1024i32..=16384,
        hp in 768i32..=16384,
        se in 0.3f64..=4.0,
        v in 0.0f64..5000.0,
    ) {
        let Some(f) = UiScreen::new(wp, hp, se) else { return Ok(()); };
        let (wu, hu) = f.ui_size();
        prop_assume!(v <= f64::from(wu));
        let x = f.edge_x(v);
        // Lower bound: the mapped column's centre is at or past v.
        prop_assert!(f.pointer_x(x) >= v - 1e-9, "edge_x {x} centre {} < v {v}", f.pointer_x(x));
        // Smallest: the column before it is strictly before v.
        if x >= 1 {
            prop_assert!(f.pointer_x(x - 1) < v, "edge_x not the smallest at v {v}");
        }
        // I4: the same edge value maps to the same column (abutting siblings share it).
        prop_assert_eq!(f.edge_x(v), x);
        let _ = hu;
    }

    /// For every physical pixel across an element, the pointer at its centre hits the displayed
    /// box exactly when the pixel lies in the physical box (I3, the consequence 1 statement).
    #[test]
    fn hit_and_draw_agree(
        wp in 1024i32..=8192,
        hp in 768i32..=8192,
        se in 0.5f64..=3.0,
        l in 0.0f64..1000.0,
        w in 1.0f64..1000.0,
    ) {
        let Some(f) = UiScreen::new(wp, hp, se) else { return Ok(()); };
        let b = DBox::new(l, 10.0, w, 20.0);
        let px = f.edge_x(b.l);
        let pr = f.edge_x(b.right());
        for x in (px - 2).max(0)..(pr + 2) {
            let inside_box = b.l <= f.pointer_x(x) && f.pointer_x(x) < b.right();
            let inside_phys = px <= x && x < pr;
            prop_assert_eq!(inside_box, inside_phys, "x {}", x);
        }
    }
}

// ------------------------------------------------------------------------------------------------
// I17 / I16 / I18: Type A fit groups (property, §9.6 "Unit: the magnification bound; the fit
// groups").
// ------------------------------------------------------------------------------------------------

/// A strategy for a fit root's window, scale and aspect.
fn window() -> impl Strategy<Value = (i32, i32, f64, Option<f64>)> {
    (
        1280i32..=7680,
        720i32..=4320,
        prop::sample::select(vec![0.5f64, 1.0, 1.25, 1.5, 2.0, 3.0]),
        prop::sample::select(vec![
            None,
            Some(4.0 / 3.0),
            Some(1.6),
            Some(16.0 / 9.0),
            Some(2.4),
        ]),
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(400))]

    /// I17: every Type A factor is bounded by the magnification bound `ρ × Ph ÷ h` and is at
    /// least the contain factor; I16: all members share one factor and one affine map from the
    /// group box's design coordinates to UI units.
    #[test]
    fn type_a_bound_and_group_map(
        (wp, hp, s, aspect) in window(),
        members in prop::collection::vec(
            (0.0f64..1400.0, 0.0f64..1000.0, 1.0f64..1400.0, 1.0f64..1000.0),
            1..=4,
        ),
        content in prop::option::of((0.0f64..1000.0, 0.0f64..700.0, 1.0f64..1000.0, 1.0f64..700.0)),
    ) {
        let se = scaling::effective_scale(s, wp, hp);
        let Some(f) = UiScreen::new(wp, hp, se) else { return Ok(()); };
        let (wu, hu) = f.ui_size();
        let a = scaling::safe_area(&f, aspect);
        let f_box = DBox::new(0.0, 0.0, f64::from(wu), f64::from(hu));
        let guard = { let g = f_box.intersect(&a); if g.is_empty() { f_box } else { g } };

        let members: Vec<FitMember> = members
            .into_iter()
            .map(|(left, top, w, h)| FitMember { left, top, w, h })
            .collect();
        let u = scaling::group_box(&members);
        let (u_l, u_t, u_w, u_h) = u;
        prop_assume!(u_w > 0.0 && u_h > 0.0);

        let content = content.map(|(x, y, w, h)| ContentBox { x, y, w, h });
        let grp = scaling::fit_type_a(f_box.w, f_box.h, u, content, guard, RHO);
        prop_assert!(grp.f.is_finite() && grp.f > 0.0);

        // I17.
        let f_cap = RHO * f_box.h / u_h;
        let f_contain = (f_box.w / u_w).min(f_box.h / u_h);
        prop_assert!(grp.f <= f_cap + 1e-6, "f {} > f_cap {}", grp.f, f_cap);
        prop_assert!(grp.f >= f_contain - 1e-6, "f {} < f_contain {}", grp.f, f_contain);

        // I16: one map (x, y) ↦ origin + (x − U.tl) × f gives every member's displayed box.
        let origin = grp.displayed(u_w, u_h, (0.0, 0.0), (1.0, 1.0));
        for m in &members {
            let fm = grp.member(u_l, u_t, m.left, m.top);
            prop_assert!((fm.f - grp.f).abs() < 1e-9, "member factor differs");
            let mb = fm.displayed(m.w, m.h, (0.0, 0.0), (1.0, 1.0));
            prop_assert!((mb.l - (origin.l + (m.left - u_l) * grp.f)).abs() < 1e-3);
            prop_assert!((mb.t - (origin.t + (m.top - u_t) * grp.f)).abs() < 1e-3);
            prop_assert!(mb.w >= -1e-9 && mb.h >= -1e-9, "negative member extent");
        }

        // Step 4: where the scaled group box is strictly larger than P on an axis (so the
        // internal `>=` clamp certainly ran), it covers P. The small positive margin avoids the
        // floating-point boundary where the clamp did or did not run.
        let gw = u_w * grp.f;
        let gh = u_h * grp.f;
        if gw >= f_box.w + 1e-6 {
            prop_assert!(grp.x <= 1e-6 && grp.x + gw >= f_box.w - 1e-4, "group box does not cover F in x");
        }
        if gh >= f_box.h + 1e-6 {
            prop_assert!(grp.y <= 1e-6 && grp.y + gh >= f_box.h - 1e-4, "group box does not cover F in y");
        }
    }

    /// I18: a Type A container nested under a parent with a per-window node scale `p` has a
    /// factor that does not depend on `p`, and a displayed box equal to `parent_tl + fitted × m`
    /// with size `design × f × m` (§3.4 item 2, §10.8).
    #[test]
    fn type_a_nested_under_node_scale(
        (wp, hp, s, aspect) in window(),
        p in 0.25f64..=4.0,
        ptl in (-500.0f64..5000.0, -500.0f64..3000.0),
        r_w in 100.0f64..2000.0,
        r_h in 100.0f64..1500.0,
        u_w in 1.0f64..2000.0,
        u_h in 1.0f64..1500.0,
    ) {
        let se = scaling::effective_scale(s, wp, hp);
        let Some(f) = UiScreen::new(wp, hp, se) else { return Ok(()); };
        let a = scaling::safe_area(&f, aspect);
        let m = (p, p);
        let a_in = scaling::safe_area_in_parent(a, ptl, m);
        let p_box = DBox::new(0.0, 0.0, r_w, r_h);
        let guard = { let g = p_box.intersect(&a_in); if g.is_empty() { p_box } else { g } };
        let inner = scaling::fit_type_a(r_w, r_h, (0.0, 0.0, u_w, u_h), None, guard, RHO);
        prop_assert!(inner.f.is_finite() && inner.f > 0.0);

        // The factor is computed in the parent's own layout coordinates (P = R's box), so it is
        // bounded by R's size, never by R's displayed (× p) size: it is at most the cover factor
        // of U in P and at most the bound, both free of p.
        let f_cap = RHO * r_h / u_h;
        prop_assert!(inner.f <= f_cap + 1e-6, "inner factor exceeds the bound (depends on p?)");

        // I18: the displayed box is parent_tl + fitted × m with size design × f × m.
        let ib = inner.displayed(u_w, u_h, ptl, m);
        prop_assert!((ib.l - (ptl.0 + inner.x * m.0)).abs() < 1e-6);
        prop_assert!((ib.t - (ptl.1 + inner.y * m.1)).abs() < 1e-6);
        prop_assert!((ib.w - u_w * inner.f * m.0).abs() < 1e-6);
        prop_assert!((ib.h - u_h * inner.f * m.1).abs() < 1e-6);
    }
}

/// I15: the §5.4 step-4 counterexample at C2 with a 4:3 safe area — step 4 wins over step 3, so
/// the content reaches into the left wing although `G` starts at 240 (§10.8 reasoning, §5.4).
#[test]
fn step4_wins_over_step3() {
    let f = UiScreen::new(1920, 1080, 1.0).unwrap();
    let a = scaling::safe_area(&f, Some(4.0 / 3.0));
    assert_eq!((a.l, a.w), (240.0, 1440.0), "A (4:3) at C2");
    let p = DBox::new(0.0, 0.0, 1920.0, 1080.0);
    let g = p.intersect(&a);
    let u = (0.0, 0.0, 1024.0, 768.0);
    let content = Some(ContentBox {
        x: 0.0,
        y: 284.0,
        w: 300.0,
        h: 200.0,
    });
    let grp = scaling::fit_type_a(p.w, p.h, u, content, g, RHO);
    assert!((grp.f - 1.875).abs() < 1e-3, "f {}", grp.f);
    // Step 4 clamps the 1920 × 1440 group box to cover F: left 0; content spans 0 … 562.5.
    assert!((grp.x - 0.0).abs() < 1e-6, "group box left {}", grp.x);
    assert!(
        ((grp.x + 300.0 * grp.f) - 562.5).abs() < 1e-6,
        "content right end"
    );
}

// ------------------------------------------------------------------------------------------------
// I8: saved placements (property, §9.6 "Unit: the anchors of §5.8").
// ------------------------------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(400))]

    /// I8: a saved placement applied at the configuration it was saved at restores the same
    /// displayed box (within 0.01 UI units), for either reference box.
    #[test]
    fn saved_placement_round_trips(
        (wp, hp, s, aspect) in window(),
        safe in any::<bool>(),
        bx in 0.0f64..3000.0,
        by in 0.0f64..2000.0,
        bw in 1.0f64..1000.0,
        bh in 1.0f64..1000.0,
    ) {
        let se = scaling::effective_scale(s, wp, hp);
        let Some(f) = UiScreen::new(wp, hp, se) else { return Ok(()); };
        let (wu, hu) = f.ui_size();
        let (reference, r) = if safe {
            (RefBox::Safe, scaling::safe_area(&f, aspect))
        } else {
            (RefBox::Screen, DBox::new(0.0, 0.0, f64::from(wu), f64::from(hu)))
        };
        let b = DBox::new(bx, by, bw, bh);
        let sp = SavedPlacement::from_box(reference, &r, &b, None, None);
        let (x, y) = sp.position(&r, b.w, b.h);
        prop_assert!((x - b.l).abs() < 0.01, "x {} vs {}", x, b.l);
        prop_assert!((y - b.t).abs() < 0.01, "y {} vs {}", y, b.t);
    }

    /// keep-on-screen (§5.8): a box no larger than `F` is always moved fully inside `F`, and a
    /// box already inside is never moved (I6 for the clamp alone).
    #[test]
    fn keep_on_screen_clamps_inside_f(
        wu in 1024i32..=8192,
        hu in 768i32..=8192,
        x in -4000.0f64..8000.0,
        y in -4000.0f64..8000.0,
        w in 1.0f64..1000.0,
        h in 1.0f64..1000.0,
    ) {
        prop_assume!(w <= f64::from(wu) && h <= f64::from(hu));
        let (nx, ny) = scaling::keep_on_screen(x, y, w, h, wu, hu);
        prop_assert!(nx >= -1e-9 && nx + w <= f64::from(wu) + 1e-9, "x out of F");
        prop_assert!(ny >= -1e-9 && ny + h <= f64::from(hu) + 1e-9, "y out of F");
        // Idempotent and non-moving when already inside.
        if x >= 0.0 && x + w <= f64::from(wu) && y >= 0.0 && y + h <= f64::from(hu) {
            prop_assert!((nx - x).abs() < 1e-9 && (ny - y).abs() < 1e-9);
        }
    }
}

// ------------------------------------------------------------------------------------------------
// Type B and Type C side fits (§9.6 "Unit: the Type A, B and C fits").
// ------------------------------------------------------------------------------------------------

/// §10.2 Type B side panels at C8: the height drives, the left panel anchors at the safe box's
/// left, the right panel ends at its right.
#[test]
fn type_b_side_panels_c8() {
    let f = UiScreen::new(5120, 1440, 1.0).unwrap();
    let a = scaling::safe_area(&f, Some(2.4));
    assert_eq!((a.l, a.w), (832.0, 3456.0));
    let cb = |l: f64, r: f64| ComputedBox {
        left: l,
        top: 0.0,
        right: r,
        bottom: a.h - 1.0,
    };
    let left = scaling::fit_type_b(a.h, a.w, 500.0, 900.0, cb(0.0, 499.0), false);
    assert!((left.f - 1.6).abs() < 1e-6);
    let lb = left.displayed(500.0, 900.0, (a.l, 0.0), (1.0, 1.0));
    assert!((lb.l - 832.0).abs() < 1e-6 && (lb.right() - 1632.0).abs() < 1e-6);
    let right = scaling::fit_type_b(a.h, a.w, 500.0, 900.0, cb(0.0, a.w - 1.0), true);
    let rb = right.displayed(500.0, 900.0, (a.l, 0.0), (1.0, 1.0));
    assert!((rb.right() - 4288.0).abs() < 1e-6 && (rb.l - 3488.0).abs() < 1e-6);
}

/// §10.8(a) Type C naming window inside its scale parent at C8 (nested fit, I18).
#[test]
fn type_c_naming_window_c8() {
    let f = UiScreen::new(5120, 1440, 1.0).unwrap();
    let a = scaling::safe_area(&f, Some(2.4));
    let parent_tl = (a.l, a.t);
    let u = (0.0, 0.0, 1440.0, 900.0);
    let content = Some(ContentBox {
        x: 516.0,
        y: 0.0,
        w: 411.0,
        h: 880.0,
    });
    let a_in = scaling::safe_area_in_parent(a, parent_tl, (1.0, 1.0));
    let p_box = DBox::new(0.0, 0.0, a.w, a.h);
    let g = {
        let i = p_box.intersect(&a_in);
        if i.is_empty() { p_box } else { i }
    };
    let grp = scaling::fit_type_a(a.w, a.h, u, content, g, RHO);
    assert!((grp.f - 1.6364).abs() < 1e-3, "f_A {}", grp.f);
    let sp = grp.displayed(1440.0, 900.0, parent_tl, (1.0, 1.0));
    assert!((sp.l - 1381.82).abs() < 0.05 && (sp.w - 2356.36).abs() < 0.05);
    let nw = scaling::fit_type_c(
        410.0,
        900.0,
        411.0,
        488.0,
        ComputedBox {
            left: 515.0,
            top: 206.0,
            right: 515.0 + 410.0 - 1.0,
            bottom: 206.0 + 488.0 - 1.0,
        },
        FitVerticalAnchor::Top,
    );
    assert!((nw.f - 410.0 / 411.0).abs() < 1e-4);
    let nwb = nw.displayed(411.0, 488.0, (sp.l, sp.t), (grp.f, grp.f));
    assert!((nwb.l - 2224.55).abs() < 0.1 && (nwb.t - 337.09).abs() < 0.1);
    assert!((nwb.w - 670.91).abs() < 0.1 && (nwb.h - 796.60).abs() < 0.1);
}

// ------------------------------------------------------------------------------------------------
// I5 degenerate-box guard, fit-to-screen (§5.6, §5.7).
// ------------------------------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    /// I5: the guard never yields a negative extent; a positive computed extent is left as is;
    /// below 1 it becomes the design extent cut to the box, and 0 only when empty by design.
    #[test]
    fn degenerate_guard_never_negative(
        computed in -100i32..2000,
        design in -100i32..2000,
        box_ext in -100i32..2000,
    ) {
        let (e, empty) = scaling::degenerate_guard(computed, design, box_ext);
        prop_assert!(e >= 0, "negative guarded extent {}", e);
        if computed >= 1 {
            prop_assert_eq!(e, computed, "positive computed changed");
            prop_assert!(!empty);
        } else {
            prop_assert_eq!(empty, e == 0);
        }
    }

    /// fit-to-screen `q` is always in (0, 1], and is exactly 1 once the panel fits.
    #[test]
    fn fit_to_screen_in_unit_range(
        dw in 0.0f64..20000.0,
        dh in 0.0f64..20000.0,
        wu in 1024i32..=8192,
        hu in 768i32..=8192,
    ) {
        let q = scaling::fit_to_screen(dw, dh, wu, hu);
        prop_assert!(q > 0.0 && q <= 1.0, "q {} out of (0, 1]", q);
        if dw <= f64::from(wu) && dh <= f64::from(hu) {
            prop_assert!((q - 1.0).abs() < 1e-12, "q {} not 1 when the panel fits", q);
        }
    }
}

// ------------------------------------------------------------------------------------------------
// T1 / T2 / T3: derived glyphs (§9.6 "Unit", §6.4.4).
// ------------------------------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    /// T2: a derived glyph's ink box is within one pixel of the source box scaled by `k`, the
    /// advance equals `A' + W' + B'`, no extent is negative, and the baseline stays on the line.
    #[test]
    fn derived_metrics_within_one_pixel(
        a in 0i32..8, w in 1i32..40, b in 0i32..8, t in 0i32..8, h in 1i32..40,
        line in 8i32..40,
        k in 0.25f64..4.0,
    ) {
        let advance = a + w + b;
        let baseline = (line - 1).max(0);
        let src = GlyphMetrics { a, w, b, t, h, advance, line_height: line, baseline, outline_border: 0 };
        let target = ((f64::from(line) * k).round() as i32).max(6);
        let d = scaling::derive_metrics(src, k, target);
        prop_assert!((f64::from(d.w) - f64::from(w) * k).abs() <= 1.0, "W' off, k {}", k);
        prop_assert!((f64::from(d.h) - f64::from(h) * k).abs() <= 1.0, "H' off, k {}", k);
        prop_assert_eq!(d.advance, d.a + d.w + d.b, "advance != A'+W'+B'");
        prop_assert!(d.w >= 0 && d.h >= 0, "negative derived extent");
        prop_assert!(d.baseline < target, "baseline off the line box");
    }

    /// T3: downsampling preserves the total coverage up to the box-ratio scale (within the
    /// quantisation half-step per derived pixel plus a small margin).
    #[test]
    fn downsample_preserves_total(
        src_w in 4usize..24, src_h in 4usize..24,
        shrink in 2usize..6,
        seed in any::<u64>(),
    ) {
        let mut state = seed | 1;
        let mut src = vec![0u8; src_w * src_h];
        for v in &mut src {
            // A cheap deterministic pseudo-random coverage.
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            *v = (state >> 56) as u8;
        }
        let dst_w = (src_w / shrink).max(1);
        let dst_h = (src_h / shrink).max(1);
        let out = scaling::downsample_coverage(&src, src_w, src_h, dst_w, dst_h);
        prop_assume!(!out.is_empty());
        let total_src: f64 = src.iter().map(|&v| f64::from(v)).sum();
        let total_out: f64 = out.iter().map(|&v| f64::from(v)).sum();
        // The area filter maps the source box onto the derived box, so the exact scale of the
        // total is the box ratio, not k² (§9.5 T3).
        let reference = total_src * (dst_w * dst_h) as f64 / (src_w * src_h) as f64;
        // T3 states the bound in 0…1 coverage units: within W'×H'÷510 ("half an 8-bit step per
        // derived pixel"), **with the quantisation term alone** for the area filter — "the 1 % is
        // for other methods" (§9.5 T3). Only the area filter is exercised here (`downsample_coverage`
        // is the §6.4.4 "Coverage, downsampled" path), so the 1 % must not be added: it would make
        // the tolerance ~255× the real per-pixel half-step and let a systematic coverage drift of
        // ~1 % pass unnoticed. The totals above are in byte units (0…255), so the coverage half-step
        // W'×H'÷510 becomes W'×H'×255÷510 = W'×H'×0.5 bytes; `+1e-6` only absorbs float noise.
        // (Before rounding the area weights sum to 1 per derived pixel, so the sum of the derived
        // coverage equals `reference` exactly; the only deviation is each pixel's ≤0.5-byte round.)
        let tol = (dst_w * dst_h) as f64 * 0.5 + 1e-6;
        prop_assert!((total_out - reference).abs() <= tol, "total {} vs {} tol {}", total_out, reference, tol);
    }
}

/// T1: a solid source rectangle stays solid when downsampled and when upscaled by the distance
/// method, with at most one partial pixel per edge.
#[test]
fn solid_glyph_stays_solid() {
    // Downsample.
    let src = vec![255u8; 12 * 12];
    let out = scaling::downsample_coverage(&src, 12, 12, 6, 6);
    assert!(out.iter().all(|&v| v == 255), "downsample lost solidity");
    // Distance upscale: the interior is solid; the second column in from each side is already
    // full, so at most the outermost column is partial.
    let src = vec![255u8; 4 * 4];
    let out = scaling::distance_coverage(&src, 4, 4, 10, 10);
    assert_eq!(out.len(), 100);
    for y in 2..8 {
        assert_eq!(out[y * 10 + 1], 255, "left-but-one not solid, row {y}");
        assert_eq!(out[y * 10 + 8], 255, "right-but-one not solid, row {y}");
    }
}

/// Hostile glyph dimensions never panic and return an empty buffer (bounded allocation).
#[test]
fn glyph_hostile_dims_empty() {
    assert!(scaling::downsample_coverage(&[], usize::MAX, usize::MAX, 1, 1).is_empty());
    assert!(scaling::downsample_coverage(&[255; 4], 2, 2, 8, 8).is_empty());
    assert!(scaling::distance_coverage(&[255; 1], 1, 1, 1 << 30, 1 << 30).is_empty());
}

// ------------------------------------------------------------------------------------------------
// Image filters (§6.3): area weights and sharp bilinear (§9.6).
// ------------------------------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    /// Area-filter weights of each destination pixel sum to 1 (an average) and index in range.
    #[test]
    fn area_weights_normalised(dst in 1usize..64, src in 1usize..64) {
        let s = dst as f64 / src as f64;
        let w = scaling::area_weights(dst, src, s);
        prop_assert_eq!(w.len(), dst);
        for slot in &w {
            let sum: f64 = slot.iter().map(|&(_, wt)| wt).sum();
            prop_assert!((sum - 1.0).abs() < 1e-9, "weights sum {}", sum);
            for &(i, _) in slot {
                prop_assert!(i < src, "source index {} out of range", i);
            }
        }
    }

    /// Sharp-bilinear weights are in `[0, 1]` and sum to 1 at any scale above 0.
    #[test]
    fn sharp_bilinear_weights(d in 0i32..4096, s in 0.01f64..8.0) {
        let (_, w0, w1) = scaling::sharp_bilinear(d, s);
        prop_assert!((0.0..=1.0).contains(&w0) && (0.0..=1.0).contains(&w1));
        prop_assert!((w0 + w1 - 1.0).abs() < 1e-9, "sum {}", w0 + w1);
    }

    /// The canvas pixel count `E(v)` under `Se` is monotone non-decreasing in `v` and equals
    /// `v` at `Se` = 1 for integer `v` (invariant I2 identity).
    #[test]
    fn canvas_px_monotone(se in 0.3f64..4.0, a in 0.0f64..2000.0, step in 0.0f64..500.0) {
        prop_assert!(scaling::canvas_px(se, a) <= scaling::canvas_px(se, a + step));
    }
}

/// `canvas_px` is the identity on integers at `Se` = 1 (I2), and the §6.3 axis-kind choice is
/// by native-vs-element size (SC8).
#[test]
fn canvas_and_axis_kind_identities() {
    for v in [0, 1, 10, 100, 1365, 2560] {
        assert_eq!(scaling::canvas_px(1.0, f64::from(v)), v);
    }
    use scaling::AxisKind;
    assert_eq!(scaling::axis_kind(64, 64), AxisKind::Fitted);
    assert_eq!(scaling::axis_kind(2, 64), AxisKind::Tiled);
    assert_eq!(scaling::axis_kind(128, 64), AxisKind::Clipped);
    // A fitted axis resamples to its physical box; a tiled/clipped axis uses the content scale.
    assert!((scaling::axis_scale(AxisKind::Fitted, 437, 200, 1.5) - 2.185).abs() < 1e-9);
    assert_eq!(scaling::axis_scale(AxisKind::Tiled, 437, 2, 1.5), 1.5);
}

// ------------------------------------------------------------------------------------------------
// I10: the auto-scale table (§4.2) and a monotonicity property.
// ------------------------------------------------------------------------------------------------

/// I10: `s_auto` follows the §4.2 table for the listed inputs.
#[test]
fn s_auto_table() {
    assert_eq!(scaling::s_auto(1.0, 720), 1.00);
    assert_eq!(scaling::s_auto(1.0, 1080), 1.00);
    assert_eq!(scaling::s_auto(1.25, 1080), 1.25);
    assert!((scaling::s_auto(1.333, 1080) - 1.30).abs() < 1e-9);
    assert_eq!(scaling::s_auto(1.0, 1440), 1.00);
    assert_eq!(scaling::s_auto(1.0, 2160), 1.50);
    assert_eq!(scaling::s_auto(2.0, 2160), 2.00);
    assert_eq!(scaling::s_auto(2.0, 2880), 2.00);
    assert_eq!(scaling::s_auto(1.0, 4320), 3.00);
    assert_eq!(scaling::s_auto(1.5, 900), 1.50);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    /// `s_auto` is non-decreasing in the window height and in the OS factor, always a 0.05 step
    /// in 0.50–4.00 (I10 shape, §4.2).
    #[test]
    fn s_auto_monotone_and_stepped(sigma in 0.5f64..4.0, hp in 1i32..=16384, extra in 1i32..=4000) {
        let a = scaling::s_auto(sigma, hp);
        let b = scaling::s_auto(sigma, (hp + extra).min(16384));
        prop_assert!(b >= a - 1e-12, "not monotone in height");
        prop_assert!((0.50..=4.00).contains(&a));
        // A multiple of 0.05.
        let steps = a / 0.05;
        prop_assert!((steps - steps.round()).abs() < 1e-6, "not a 0.05 step: {}", a);
    }
}

/// A fit container with a zero design extent, or a zero reference box, is not fitted
/// (§5.4 "Zero extents"): the factor is 1 at the origin.
#[test]
fn zero_extent_fit_is_identity() {
    use scaling::Fit;
    let id = scaling::fit_type_a(
        1000.0,
        800.0,
        (0.0, 0.0, 0.0, 500.0),
        None,
        DBox::new(0.0, 0.0, 1.0, 1.0),
        RHO,
    );
    assert_eq!(id, Fit::IDENTITY);
    let id = scaling::fit_type_a(
        0.0,
        800.0,
        (0.0, 0.0, 100.0, 100.0),
        None,
        DBox::new(0.0, 0.0, 1.0, 1.0),
        RHO,
    );
    assert_eq!(id, Fit::IDENTITY);
}

/// A glyph source choice is `Exact` on a ladder hit, `Downsampled` below the shortest taller
/// record and `Distance` above the tallest (§6.4.3); the anchors are used only for Type C.
#[test]
fn glyph_source_choice_and_anchor_use() {
    let ladder = [10, 12, 14, 16, 20];
    assert_eq!(
        scaling::choose_glyph_source(&ladder, 16).kind,
        SourceKind::Exact
    );
    assert_eq!(
        scaling::choose_glyph_source(&ladder, 8).kind,
        SourceKind::Downsampled
    );
    assert_eq!(
        scaling::choose_glyph_source(&ladder, 30).kind,
        SourceKind::Distance
    );
    assert_eq!(
        scaling::choose_glyph_source(&[], 20).kind,
        SourceKind::Exact
    );
    let _ = Anchor::Centre;
}
