// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-ui` UI-scaling fit computation (`ui-scaling.md` §3–§6, decision 0040): an
//! arbitrary window size (including zero sides and the 16,384 clamp bound), user scale (any
//! value, finite or not), OS scale factor, safe-aspect ratio and a small synthetic layout of
//! fit containers (a Type A group of several members with a content box, a Type B side panel,
//! a Type C container, a per-window node scale and a nested Type A under it) are run through
//! the whole pure geometry of `tessera_ui::scaling`. Nothing may panic, divide by zero, loop
//! without end or allocate without bound, and the invariants of `ui-scaling.md` §9.5 that the
//! pure core can establish hold on every input:
//!
//! - **I1** `Wu ≥ 1024`, `Hu ≥ 768` and `Se ≤ S` (the resolved scale) and `Se > 0`; a zero-sided
//!   window has no screen and nothing is computed from it (`UiScreen::new` is `None`). A sequence
//!   of sizes including zero sides is driven (below), and the pure-core half of **part 2** is
//!   checked: a zero side computes nothing and leaves the retained last-good state intact.
//! - **I5** every fit factor is finite and positive and no displayed box has a negative extent.
//! - **I6** (proxy) a root run through keep-on-screen lies inside `F`.
//! - **I14** (pure-core slice) recomputing a size's screen, safe area and Type A fit is
//!   byte-identical, including after an intervening different size (`A → B → A`).
//! - **I16** all members of a Type A fit group share one factor `f` and one map from the group
//!   box to UI units, so the same design point of two members lands at the same place.
//! - **I17** every Type A factor satisfies `f ≤ ρ × Ph ÷ h` and `f ≥ f_contain`.
//! - **I18** a fit container nested under a node scale has the displayed box of §3.4 item 2 and
//!   a factor that does not depend on the factor of the container above it; a Type C fitted inside
//!   a Type A member box (a B/C inside a Type A, §9.6) takes its factor from the member box alone.
//!
//! **Stateful mode (the live layout host).** The same input also drives a live
//! [`tessera_ui::Ui`] built from synthetic layouts (two layered fit roots of one layout, a
//! full-screen (5, 5, 5, 5) root with a Type A backdrop, a Type B side panel and a safe-box
//! child, a safe-area panel with a node scale, and a plugin window; extents may be 0) through a
//! sequence of window sizes, zero sides, minimise/restore, shows and hides, drags and node-scale
//! changes, and asserts the stateful halves of §9.6: a size with a zero side leaves `Se`, `F`,
//! `A` and every displayed box, fit factor and fit-to-screen factor unchanged (I1 part 2); a
//! restore to the same size moves nothing (I1, I14: laying out the same configuration again
//! gives the same boxes); the two fit roots always share one factor and one displayed box,
//! shown or not (I16); every fit factor is finite and positive and no layout box has a negative
//! extent (I5); after a screen change every shown native root that fits lies inside `F` (I6).
//!
//! The synthetic glyph path (`target_height`, the ladder choice and a small bounded coverage
//! derivation) is exercised too, so the downsample and distance filters see fuzzed heights
//! without ever allocating from an unchecked dimension.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::did::Did;
use tessera_formats::property::{PropertyEntry, PropertyValue};
use tessera_formats::ui_layout::{EdgeMode, ElementDesc, UiLayout};
use tessera_ui::scaling::{
    self, Anchor, ComputedBox, ContentBox, DBox, Fit, FitMember, FitVerticalAnchor, GlyphMetrics,
    RHO, RefBox, SavedPlacement, SourceKind, UiScreen,
};
use tessera_ui::{ControlKind, ElementKey, IBox, MemoryEnv, Ui, Vocab};

/// A little-endian byte reader that never runs out: past the end it reads zero.
struct Bytes<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Bytes<'_> {
    fn u8(&mut self) -> u8 {
        let b = self.data.get(self.pos).copied().unwrap_or(0);
        self.pos = self.pos.saturating_add(1);
        b
    }

    fn u16(&mut self) -> u16 {
        u16::from_le_bytes([self.u8(), self.u8()])
    }

    fn u32(&mut self) -> u32 {
        u32::from_le_bytes([self.u8(), self.u8(), self.u8(), self.u8()])
    }

    fn u64(&mut self) -> u64 {
        let mut v = [0u8; 8];
        for b in &mut v {
            *b = self.u8();
        }
        u64::from_le_bytes(v)
    }

    /// A window side: usually a sensible pixel count (which includes 0 for a zero side), and
    /// occasionally a wild value so the 16,384 clamp and the i32 edges are exercised.
    fn side(&mut self) -> i32 {
        match self.u8() % 8 {
            0 => 0,                              // a zero side (minimised, §3.2)
            1 => self.u32() as i32,              // anything, including negatives and > 16,384
            _ => i32::from(self.u16()) % 20_001, // 0 … 20,000
        }
    }

    /// A scale: usually a value around the UI-scale range, sometimes raw bits so non-finite and
    /// out-of-range values reach the clamps.
    fn scale(&mut self) -> f64 {
        match self.u8() % 4 {
            0 => f64::from_bits(self.u64()),
            _ => 0.1 + f64::from(self.u16()) / 10_000.0, // 0.1 … ~6.65
        }
    }

    /// A design-space coordinate: a bounded **finite** value in -32,768 … 32,767, which may be
    /// negative (an element placed above or left of its parent). Layout numbers are integers
    /// from the records (`ui-scaling.md` §3.3), so this reader stays finite and integer valued;
    /// the hostile fuzzing is on the window, scale, aspect and safe-aspect inputs.
    fn coord(&mut self) -> f64 {
        f64::from(self.u16() as i32 - 32_768)
    }

    /// A design-space extent: a non-negative **finite**, integer-valued size in 0 … 65,535,
    /// including 0 so the zero-extent and degenerate paths (§5.4 "Zero extents", §5.7) are
    /// exercised. Design extents are integers (`ui-scaling.md` §3.3), never subnormal reals.
    fn extent(&mut self) -> f64 {
        f64::from(self.u16())
    }
}

/// The safe-aspect choices of §5.1 (`None` is `"full"`). Reads from the byte stream so the
/// exotic branch can feed `safe_area` a **full** 64-bit pattern (NaN, ±∞, a huge or a subnormal
/// ratio), exercising its non-finite guard and the 4⁄3 … 4.0 clamp under hostile input. A
/// selector byte derived only from a `u8` could never leave the mantissa, so it never produced a
/// non-finite or out-of-range value and `safe_area`'s "treat non-finite as full" path went
/// untested.
fn aspect(r: &mut Bytes) -> Option<f64> {
    match r.u8() % 7 {
        0 => None,
        1 => Some(4.0 / 3.0),
        2 => Some(1.6),
        3 => Some(16.0 / 9.0),
        4 => Some(2.4),
        5 => Some(0.1 + f64::from(r.u16()) / 100.0), // an unclamped custom ratio, 0.1 … ~655
        _ => Some(f64::from_bits(r.u64())),          // exotic: NaN, ±∞, huge or subnormal
    }
}

/// Checks a displayed box never carries a negative extent (I5).
fn check_box(b: &DBox, what: &str) {
    assert!(b.w >= -1e-6, "{what}: negative width {}", b.w);
    assert!(b.h >= -1e-6, "{what}: negative height {}", b.h);
    assert!(
        b.l.is_finite() && b.t.is_finite(),
        "{what}: non-finite origin"
    );
}

fuzz_target!(|data: &[u8]| {
    pure(data);
    stateful(data);
});

/// The pure geometry of `tessera_ui::scaling` on one input.
fn pure(data: &[u8]) {
    let mut r = Bytes { data, pos: 0 };

    // ----- the window, the scale and the screen F (§3.2, §4.2, §4.3) --------------------------
    let s = r.scale();
    let sigma = r.scale();
    let wp = r.side();
    let hp = r.side();
    let asp = aspect(&mut r);

    // s_auto never panics for any sigma/height and stays in range.
    let auto = scaling::s_auto(sigma, hp);
    assert!(
        (0.50..=4.00).contains(&auto) && auto.is_finite(),
        "s_auto out of range: {auto}"
    );
    // The resolved scale: a number, or the auto default. round_scale clamps hostile input.
    let resolved = if r.u8().is_multiple_of(2) {
        scaling::round_scale(s)
    } else {
        auto
    };
    let se = scaling::effective_scale(resolved, wp, hp);
    assert!(
        se.is_finite() && se > 0.0,
        "Se must be positive finite: {se}"
    );
    // Se never exceeds the resolved scale (I1), up to a tiny rounding margin.
    assert!(se <= resolved + 1e-9, "Se {se} > resolved {resolved}");

    let Some(screen) = UiScreen::new(wp, hp, se) else {
        // A zero-sided (or otherwise degenerate) window has no screen: nothing else runs (§3.2).
        return;
    };
    let (wu, hu) = screen.ui_size();
    // I1: the design floor.
    assert!(
        wu >= 1024 && hu >= 768,
        "F below the design floor: {wu}x{hu}"
    );
    // F covers the window exactly, no strip (§3.2).
    assert!(
        screen.kx() > 0.0 && screen.ky() > 0.0,
        "axis factors positive"
    );

    // The pixel-centre mapping agrees with its inverse at a few columns (I3): the column of an
    // edge, mapped back to a pointer, lies at or past the edge; the column before it does not.
    for &v in &[0.0, f64::from(wu) / 3.0, f64::from(wu) - 0.5] {
        let x = screen.edge_x(v);
        if x >= 1 {
            assert!(screen.pointer_x(x) >= v - 1e-9, "edge_x not a lower bound");
            assert!(
                screen.pointer_x(x - 1) < v + 1e-9,
                "edge_x not the smallest"
            );
        }
    }

    // ----- the safe area A (§5.1) -------------------------------------------------------------
    let a = scaling::safe_area(&screen, asp);
    check_box(&a, "safe area");
    // §5.1: `A` is never narrower than 1024 UI units (Hu ≥ 768, a clamped to ≥ 4⁄3, so
    // Aw ≥ round(768 × 4⁄3) = 1024; "full"/non-finite gives Aw = Wu ≥ 1024). Match the tight
    // bound of the sibling invariants test, not a looser 1023 that would admit an off-by-one.
    assert!(a.w >= 1024.0, "A narrower than 1024: {}", a.w);
    let f_box = DBox::new(0.0, 0.0, f64::from(wu), f64::from(hu));
    assert!(a.inside(&f_box, 1e-6), "A not inside F");

    // ----- a Type A fit-root group of several members with a content box (§5.4) ---------------
    let member_count = (r.u8() % 4) as usize + 1;
    let mut members = Vec::with_capacity(member_count);
    for _ in 0..member_count {
        members.push(FitMember {
            left: r.coord(),
            top: r.coord(),
            w: r.extent(),
            h: r.extent(),
        });
    }
    let u = scaling::group_box(&members);
    let content = if r.u8().is_multiple_of(2) {
        Some(ContentBox {
            x: r.coord(),
            y: r.coord(),
            w: r.extent(),
            h: r.extent(),
        })
    } else {
        None
    };
    // The guard box: F ∩ A (or F when empty), as a fit root uses (m(parent) = 1, origin 0).
    let guard = {
        let g = f_box.intersect(&a);
        if g.is_empty() { f_box } else { g }
    };
    let grp = scaling::fit_type_a(f_box.w, f_box.h, u, content, guard, RHO);
    assert!(
        grp.f.is_finite() && grp.f > 0.0,
        "group f not positive finite"
    );

    let (u_l, u_t, u_w, u_h) = u;
    if u_w > 0.0 && u_h > 0.0 && u_w.is_finite() && u_h.is_finite() {
        // I17: the magnification bound and the contain floor.
        let f_cap = RHO * f_box.h / u_h;
        let f_contain = (f_box.w / u_w).min(f_box.h / u_h);
        assert!(grp.f <= f_cap + 1e-6, "I17: f {} > f_cap {f_cap}", grp.f);
        assert!(
            grp.f >= f_contain - 1e-6,
            "I17: f {} < f_contain {f_contain}",
            grp.f
        );

        // I16: every member shares the factor and one affine map from U's design coordinates to
        // UI units. The map is (x, y) ↦ group top-left + (x − U.left, y − U.top) × f.
        let origin = grp.displayed(u_w, u_h, (0.0, 0.0), (1.0, 1.0));
        for m in &members {
            if m.w <= 0.0 || m.h <= 0.0 || !m.w.is_finite() || !m.h.is_finite() {
                continue;
            }
            let fm = grp.member(u_l, u_t, m.left, m.top);
            assert!((fm.f - grp.f).abs() < 1e-9, "I16: member factor differs");
            let mb = fm.displayed(m.w, m.h, (0.0, 0.0), (1.0, 1.0));
            check_box(&mb, "member box");
            let want_l = origin.l + (m.left - u_l) * grp.f;
            let want_t = origin.t + (m.top - u_t) * grp.f;
            assert!((mb.l - want_l).abs() < 1e-3, "I16: member left off the map");
            assert!((mb.t - want_t).abs() < 1e-3, "I16: member top off the map");
        }
    }

    // ----- a nested Type A under a per-window node scale (§3.4 item 2, §10.8) -----------------
    // A safe-area root R with a per-window factor p, dragged to some displayed box; a Type A
    // group of one lives inside it. I18: the inner fit's factor and displayed box come from the
    // parent's coordinates and do not depend on the parent's factor being folded in twice.
    let p = (0.25 + f64::from(r.u16()) / 10_000.0).min(8.0); // 0.25 … ~6.8
    let parent_tl = (r.coord(), r.coord());
    let m = (p, p);
    let a_in = scaling::safe_area_in_parent(a, parent_tl, m);
    check_box(&a_in, "A in parent");
    let r_w = r.extent().max(1.0);
    let r_h = r.extent().max(1.0);
    let p_box = DBox::new(0.0, 0.0, r_w, r_h);
    let inner_guard = {
        let g = p_box.intersect(&a_in);
        if g.is_empty() { p_box } else { g }
    };
    let inner_u = (0.0, 0.0, r.extent().max(1.0), r.extent().max(1.0));
    let inner = scaling::fit_type_a(r_w, r_h, inner_u, None, inner_guard, RHO);
    assert!(
        inner.f.is_finite() && inner.f > 0.0,
        "inner f not positive finite"
    );
    // I18: the displayed box equals parent_tl + fitted × m with size design × f × m.
    let (_, _, iw, ih) = inner_u;
    let ib = inner.displayed(iw, ih, parent_tl, m);
    check_box(&ib, "inner box");
    if m.0.is_finite() && m.1.is_finite() {
        assert!(
            (ib.l - (parent_tl.0 + inner.x * m.0)).abs() < 1e-3,
            "I18: left"
        );
        assert!((ib.w - iw * inner.f * m.0).abs() < 1e-3, "I18: width");
    }

    // ----- Type B side panel and Type C container (§5.4) --------------------------------------
    let b_fit = scaling::fit_type_b(
        a.h,
        a.w,
        r.extent().max(1.0),
        r.extent().max(1.0),
        ComputedBox {
            left: r.coord(),
            top: r.coord(),
            right: r.coord(),
            bottom: r.coord(),
        },
        r.u8().is_multiple_of(2),
    );
    assert!(b_fit.f >= 0.0 && b_fit.f.is_finite(), "Type B f");
    let c_anchor = match r.u8() % 3 {
        0 => FitVerticalAnchor::Top,
        1 => FitVerticalAnchor::Bottom,
        _ => FitVerticalAnchor::Centre,
    };
    let c_fit = scaling::fit_type_c(
        a.w,
        a.h,
        r.extent().max(1.0),
        r.extent().max(1.0),
        ComputedBox {
            left: r.coord(),
            top: r.coord(),
            right: r.coord(),
            bottom: r.coord(),
        },
        c_anchor,
    );
    assert!(c_fit.f >= 0.0 && c_fit.f.is_finite(), "Type C f");

    // ----- a Type C nested inside a Type A member box (§5.4 "a Type B or C inside a Type A") --
    // §9.6 asks for "a Type B or C inside a Type A". In the pure core that nesting is a Type C
    // fitted against the *displayed box* of a Type A member (the member box is its reference box).
    // Fit it and check it never panics and stays finite; the inner factor comes from the member
    // box alone and so does not depend on the Type A factor being folded in twice (the pure-core
    // slice of I18 for a B/C under an A).
    if let Some(m0) = members.first()
        && m0.w > 0.0
        && m0.h > 0.0
        && m0.w.is_finite()
        && m0.h.is_finite()
    {
        let mbox =
            grp.member(u.0, u.1, m0.left, m0.top)
                .displayed(m0.w, m0.h, (0.0, 0.0), (1.0, 1.0));
        if mbox.w > 0.0 && mbox.h > 0.0 {
            let nested_c = scaling::fit_type_c(
                mbox.w,
                mbox.h,
                r.extent().max(1.0),
                r.extent().max(1.0),
                ComputedBox {
                    left: r.coord(),
                    top: r.coord(),
                    right: r.coord(),
                    bottom: r.coord(),
                },
                c_anchor,
            );
            assert!(
                nested_c.f >= 0.0 && nested_c.f.is_finite(),
                "nested Type C f"
            );
        }
    }

    // ----- a window-size sequence with zero sides, and re-layout determinism ------------------
    // §9.6 asks this target to run "sequences of window sizes that include zero sides". The pure
    // core establishes here that a zero-sided window yields no screen (nothing is computed from
    // it, I1 part 2) and that recomputing a size's geometry — including after an intervening
    // different size, A→B→A — is byte-identical (the deterministic slice of I14). `kept` stands
    // in for a retained last-good state; the live tree's retained state is checked by
    // `stateful` below.
    let a_sz = (r.side(), r.side());
    let b_sz = (r.side(), r.side());
    let seq = [a_sz, (0, a_sz.1), (a_sz.0, 0), (0, 0), b_sz, a_sz];
    let mut kept: Option<(UiScreen, DBox, Fit)> = None;
    let mut first_a: Option<(UiScreen, DBox, Fit)> = None;
    for (i, &(sw, sh)) in seq.iter().enumerate() {
        match UiScreen::new(sw, sh, se) {
            None => {
                // A zero-sided (or degenerate) window computes nothing; the retained state stands.
                // The only such sizes in the sequence are the three with a zero side; a valid
                // `se` and non-zero sides always make a screen, so `kept` is never dropped here.
            }
            Some(sc) => {
                let sa = scaling::safe_area(&sc, asp);
                let fit = scaling::fit_type_a(
                    f64::from(sc.ui_size().0),
                    f64::from(sc.ui_size().1),
                    u,
                    content,
                    {
                        let fb = DBox::new(
                            0.0,
                            0.0,
                            f64::from(sc.ui_size().0),
                            f64::from(sc.ui_size().1),
                        );
                        let g = fb.intersect(&sa);
                        if g.is_empty() { fb } else { g }
                    },
                    RHO,
                );
                // Determinism: recomputing the very same size gives byte-identical results.
                let again = scaling::safe_area(&sc, asp);
                assert_eq!(sa, again, "safe_area not deterministic");
                let state = (sc, sa, fit);
                // A→B→A: the first and last entries of `seq` are the same size `a_sz`; they must
                // produce the identical screen, safe area and fit regardless of the sizes between.
                if i == 0 {
                    first_a = Some(state);
                } else if i == seq.len() - 1
                    && let Some(f0) = first_a
                {
                    assert_eq!(f0.0, state.0, "I14: A→B→A screen differs");
                    assert_eq!(f0.1, state.1, "I14: A→B→A safe area differs");
                    assert_eq!(f0.2, state.2, "I14: A→B→A fit differs");
                }
                kept = Some(state);
            }
        }
    }
    // The retained state, if any size in the sequence was valid, is still a usable screen: the
    // zero sides never corrupted it.
    if let Some((sc, _, _)) = kept {
        let (kw, kh) = sc.ui_size();
        assert!(kw >= 1024 && kh >= 768, "retained screen below the floor");
    }

    // ----- fit-to-screen and keep-on-screen (§5.6, §5.8) --------------------------------------
    let disp_w = r.extent();
    let disp_h = r.extent();
    let q = scaling::fit_to_screen(disp_w, disp_h, wu, hu);
    assert!((0.0..=1.0).contains(&q) && q > 0.0, "q out of (0, 1]: {q}");
    // A root sized by q then kept on screen lies inside F (I6 proxy). Only when it actually
    // fits (both extents ≤ F), which q guarantees for finite positive extents.
    if disp_w.is_finite() && disp_h.is_finite() && disp_w > 0.0 && disp_h > 0.0 {
        let (kw, kh) = (disp_w * q, disp_h * q);
        let (kx, ky) = scaling::keep_on_screen(r.coord(), r.coord(), kw, kh, wu, hu);
        if kw <= f64::from(wu) + 1e-6 && kh <= f64::from(hu) + 1e-6 {
            let kept = DBox::new(kx, ky, kw, kh);
            assert!(kept.inside(&f_box, 1e-3), "I6: kept root outside F");
        }
    }

    // ----- a saved placement round-trips at the configuration it was saved at (I8) ------------
    let saved_box = DBox::new(r.coord(), r.coord(), r.extent(), r.extent());
    let reference = if r.u8().is_multiple_of(2) {
        (RefBox::Screen, f_box)
    } else {
        (RefBox::Safe, a)
    };
    if saved_box.w.is_finite() && saved_box.h.is_finite() {
        let sp = SavedPlacement::from_box(reference.0, &reference.1, &saved_box, None, None);
        let (px, py) = sp.position(&reference.1, saved_box.w, saved_box.h);
        assert!(px.is_finite() && py.is_finite(), "placement not finite");
    }

    // ----- the glyph path with fuzzed heights (§6.4), bounded coverage ------------------------
    let line_height = i32::from(r.u16() % 400);
    let c = r.scale();
    let x = r.scale();
    let t = scaling::target_height(line_height, c, x);
    assert!(t >= 6, "target height below the floor: {t}");
    // A synthetic five-record ladder; the choice never panics and its k is finite.
    let ladder = [10, 12, 14, 16, 20];
    let src = scaling::choose_glyph_source(&ladder, t);
    assert!(
        src.k.is_finite() && src.k > 0.0,
        "glyph k not positive finite"
    );
    // Derive a small bounded coverage on whichever path the source kind picks, with dimensions
    // tiny enough that the allocation is trivially bounded (the functions also guard internally).
    let sw = 6usize;
    let sh = 6usize;
    let srccov = vec![200u8; sw * sh];
    let (dw, dh) = match src.kind {
        SourceKind::Downsampled => (3, 3),
        SourceKind::Distance => (12, 12),
        SourceKind::Exact => (sw, sh),
    };
    let cov = match src.kind {
        SourceKind::Downsampled => scaling::downsample_coverage(&srccov, sw, sh, dw, dh),
        SourceKind::Distance => scaling::distance_coverage(&srccov, sw, sh, dw, dh),
        SourceKind::Exact => srccov.clone(),
    };
    assert!(cov.len() <= dw * dh, "coverage over its box");
    // Metric derivation never produces negative extents or an advance that disagrees.
    let gm = GlyphMetrics {
        a: 1,
        w: 8,
        b: 2,
        t: 1,
        h: 10,
        advance: 11,
        line_height: src.source_height,
        baseline: src.source_height - 1,
        outline_border: 0,
    };
    let dm = scaling::derive_metrics(gm, src.k, t);
    assert!(dm.w >= 0 && dm.h >= 0, "derived metrics negative");
    assert_eq!(dm.advance, dm.a + dm.w + dm.b, "advance = A + W + B");

    // cursor sizing never panics and stays positive.
    let cur = scaling::cursor_size(se, r.scale());
    assert!(cur >= 1, "cursor side below 1");
    let _ = (Fit::IDENTITY, Anchor::Centre);
}

// ----- the stateful mode: a live `Ui` through a size sequence (§9.6) ---------------------------

const L_THEATRE: u32 = 0x22F0_0001;
const L_SCREEN: u32 = 0x22F0_0002;
const L_PANEL: u32 = 0x22F0_0003;
const E_LOWER: u32 = 0x1000_0001;
const E_UPPER: u32 = 0x1000_0002;
const E_SCREEN: u32 = 0x1000_0010;
const E_PANEL: u32 = 0x1000_0020;

/// Most operations of one stateful run (Tessera fuzz bound).
const MAX_OPS: usize = 24;

/// A field element at (x, y), w × h, with edge modes `e`.
fn el(id: u32, x: i32, y: i32, w: i32, h: i32, e: [u32; 4]) -> ElementDesc {
    let mut d = ElementDesc {
        element_id: id,
        element_type: Vocab::default().types.field,
        ..ElementDesc::default()
    };
    d.block.rect.x = x;
    d.block.rect.y = y;
    d.block.rect.width = w;
    d.block.rect.height = h;
    d.block.has_position = true;
    d.edges.left = EdgeMode(e[0]);
    d.edges.top = EdgeMode(e[1]);
    d.edges.right = EdgeMode(e[2]);
    d.edges.bottom = EdgeMode(e[3]);
    d
}

/// Object mode Stretch (a fit container when an edge is 6).
fn stretch(mut d: ElementDesc) -> ElementDesc {
    d.block.properties.entries.push(PropertyEntry {
        pid: Vocab::default().pids.object_mode,
        value: PropertyValue::Enum(tessera_ui::object_mode::STRETCH),
    });
    d.block.properties.size.count = d.block.properties.entries.len() as u32;
    d
}

fn with_child(mut d: ElementDesc, c: ElementDesc) -> ElementDesc {
    d.children.push(c);
    d.children_size.count = d.children.len() as u32;
    d
}

fn layout_of(did: u32, w: u32, h: u32, elements: Vec<ElementDesc>) -> UiLayout {
    let mut l = UiLayout {
        did: Did(did),
        design_width: w,
        design_height: h,
        ..UiLayout::default()
    };
    l.elements_size.count = elements.len() as u32;
    l.elements = elements;
    l
}

/// A small design extent, 0 included (zero-extent paths, §5.4 "Zero extents", §5.7).
fn small_extent(r: &mut Bytes) -> i32 {
    i32::from(r.u16() % 2_048)
}

/// A small design coordinate, negatives included.
fn small_coord(r: &mut Bytes) -> i32 {
    i32::from(r.u16() % 4_096) - 1_024
}

/// An edge mode 0–6.
fn edge(r: &mut Bytes) -> u32 {
    u32::from(r.u8() % 7)
}

/// Every element's state the stateful invariants compare.
type Snap = Vec<(IBox, Option<IBox>, f32, f32)>;

fn snap(ui: &Ui, keys: &[ElementKey]) -> Snap {
    keys.iter()
        .map(|k| {
            (
                ui.displayed_box(*k),
                ui.layout_box(*k),
                ui.fit_factor(*k),
                ui.fit_to_screen_factor(*k),
            )
        })
        .collect()
}

/// Drives a live tree through a sequence of window sizes and changes (see the module docs).
fn stateful(data: &[u8]) {
    let mut r = Bytes { data, pos: 0 };
    // Skip the bytes the pure half reads first so the two halves see different values.
    let _ = r.u64();

    // The layered fit roots of one 1024 × 768 layout (the theatre of §10.6), content fuzzed.
    let lower = with_child(
        el(E_LOWER, 0, 0, 1024, 768, [6, 5, 6, 5]),
        el(0x1000_0003, 0, 0, 1024, 768, [0, 0, 0, 0]),
    );
    let upper = with_child(
        el(E_UPPER, 0, 0, 1024, 768, [6, 5, 6, 5]),
        el(
            0x1000_0004,
            small_coord(&mut r),
            small_coord(&mut r),
            small_extent(&mut r),
            small_extent(&mut r),
            [0, 0, 0, 0],
        ),
    );
    // A full-screen (5, 5, 5, 5) root with a fuzzed design area: a Type A backdrop, a Type B side
    // panel and a non-backdrop child laid out in the safe box (§5.3).
    let (sw, sh) = (small_extent(&mut r), small_extent(&mut r));
    let backdrop = with_child(
        stretch(el(0x1000_0011, 0, 0, sw, sh, [6, 5, 6, 5])),
        el(
            0x1000_0012,
            small_coord(&mut r),
            small_coord(&mut r),
            small_extent(&mut r),
            small_extent(&mut r),
            [0, 0, 0, 0],
        ),
    );
    let side = stretch(el(
        0x1000_0013,
        0,
        0,
        small_extent(&mut r),
        sh,
        [5, 5, 6, 5],
    ));
    let safe_kid = el(
        0x1000_0014,
        small_coord(&mut r),
        small_coord(&mut r),
        small_extent(&mut r),
        small_extent(&mut r),
        [edge(&mut r), edge(&mut r), edge(&mut r), edge(&mut r)],
    );
    let screen_root = with_child(
        with_child(
            with_child(el(E_SCREEN, 0, 0, sw, sh, [5, 5, 5, 5]), backdrop),
            side,
        ),
        safe_kid,
    );
    // A safe-area panel with fuzzed edges.
    let panel = el(
        E_PANEL,
        small_coord(&mut r),
        small_coord(&mut r),
        small_extent(&mut r),
        small_extent(&mut r),
        [edge(&mut r), edge(&mut r), edge(&mut r), edge(&mut r)],
    );
    let mut env = MemoryEnv::default();
    env.add_layout(layout_of(L_THEATRE, 1024, 768, vec![lower, upper]));
    env.add_layout(layout_of(
        L_SCREEN,
        sw.max(0) as u32,
        sh.max(0) as u32,
        vec![screen_root],
    ));
    env.add_layout(layout_of(L_PANEL, 1024, 768, vec![panel]));
    let mut ui = Ui::new(Box::new(env), Vocab::default(), (1024, 768));
    let mut keys = Vec::new();
    let lo = ui.open_panel(Did(L_THEATRE), E_LOWER).ok();
    let up = ui.open_panel(Did(L_THEATRE), E_UPPER).ok();
    keys.extend(lo);
    keys.extend(up);
    if let Ok(k) = ui.open_panel(Did(L_SCREEN), E_SCREEN) {
        keys.push(k);
        for id in [0x1000_0011, 0x1000_0013, 0x1000_0014] {
            keys.extend(ui.find_child(k, id));
        }
    }
    let panel = ui.open_panel(Did(L_PANEL), E_PANEL).ok();
    keys.extend(panel);
    let plugin = ui.create_control(ControlKind::Window, 1).ok();
    if let Some(w) = plugin {
        ui.control_set_size(w, small_extent(&mut r), small_extent(&mut r));
        ui.control_set_position(w, small_coord(&mut r), small_coord(&mut r));
        ui.control_set_visible(w, true);
        keys.push(w);
    }

    let mut current: Option<(UiScreen, DBox)> = None;
    let ops = (r.u8() as usize % MAX_OPS) + 1;
    for _ in 0..ops {
        match r.u8() % 6 {
            // A window size: a zero side changes nothing (I1 part 2); otherwise §4.5 runs.
            0 | 1 => {
                let (wp, hp) = (r.side(), r.side());
                let s = 0.5 + f64::from(r.u8()) / 64.0;
                let se = scaling::effective_scale(s, wp, hp);
                let asp = aspect(&mut r);
                match UiScreen::new(wp, hp, se) {
                    None => {
                        let before = snap(&ui, &keys);
                        let (scr, safe, size) =
                            (ui.ui_screen(), ui.safe_area_box(), ui.screen_size());
                        ui.resize_screen(wp, hp);
                        if current.is_some() {
                            assert_eq!(ui.ui_screen(), scr, "I1: Se/F kept at a zero side");
                            assert_eq!(ui.safe_area_box(), safe, "I1: A kept at a zero side");
                            assert_eq!(ui.screen_size(), size, "I1: F kept at a zero side");
                            assert_eq!(snap(&ui, &keys), before, "I1: boxes kept at a zero side");
                        }
                    }
                    Some(f) => {
                        let a = scaling::safe_area(&f, asp);
                        let changed = current != Some((f, a));
                        ui.set_ui_screen(f, a);
                        current = Some((f, a));
                        if changed {
                            check_inside(&ui, &keys, &[plugin, lo, up]);
                        }
                    }
                }
            }
            // Minimise then restore to the same size: nothing moves (I1, I14).
            2 => {
                if let Some((f, a)) = current {
                    let before = snap(&ui, &keys);
                    ui.resize_screen(0, 0);
                    ui.set_ui_screen(f, a);
                    assert_eq!(
                        snap(&ui, &keys),
                        before,
                        "I1: minimise/restore moved something"
                    );
                }
            }
            // Show or hide a fit root: no member moves (I16).
            3 => {
                if let (Some(lo), Some(up)) = (lo, up) {
                    let k = if r.u8().is_multiple_of(2) { lo } else { up };
                    let other = if k == lo { up } else { lo };
                    let before = (ui.displayed_box(other), ui.fit_factor(other));
                    let shown = ui.is_displayed(k);
                    ui.set_shown(k, !shown);
                    assert_eq!(
                        (ui.displayed_box(other), ui.fit_factor(other)),
                        before,
                        "I16: hiding a member moved another"
                    );
                }
            }
            // A drag of the safe-area panel.
            4 => {
                if let Some(p) = panel {
                    ui.move_to(p, small_coord(&mut r), small_coord(&mut r));
                }
            }
            // A node scale on the panel (§5.6 multiplies it).
            _ => {
                if let Some(p) = panel {
                    let s = 0.25 + f64::from(r.u8()) / 32.0;
                    ui.set_node_scale(p, Some([s as f32, s as f32, 1.0]));
                }
            }
        }
        // I16 after every step: the two fit roots share one factor and one displayed box.
        if let (Some(lo), Some(up)) = (lo, up) {
            assert_eq!(ui.fit_factor(lo), ui.fit_factor(up), "I16: one factor");
            assert_eq!(ui.displayed_box(lo), ui.displayed_box(up), "I16: one box");
        }
        // I5: positive finite factors, no negative extents.
        for k in &keys {
            let f = ui.fit_factor(*k);
            assert!(f.is_finite() && f > 0.0, "I5: fit factor {f}");
            if let Some(b) = ui.layout_box(*k) {
                assert!(b.w() >= 0 && b.h() >= 0, "I5: negative extent {b:?}");
            }
        }
        // The bars and the hit test never panic.
        let bars = ui.letterbox_bars();
        assert!(bars.iter().all(|g| g.rects.len() <= scaling::MAX_BAR_RECTS));
        let (fw, fh) = ui.screen_size();
        let _ = ui.hit_test(
            i32::from(r.u16()) % fw.max(1),
            i32::from(r.u16()) % fh.max(1),
        );
    }
}

/// I6 after a change of `Se`, `F` or `A`: every shown native root that is not a fit root, when
/// its displayed box fits, lies inside F (`skip`: the plugin window, checked by its own rule of
/// §5.8 item 5, and the fit roots, which are clipped to F instead).
fn check_inside(ui: &Ui, keys: &[ElementKey], skip: &[Option<ElementKey>]) {
    let (wu, hu) = ui.screen_size();
    for k in keys {
        if skip.contains(&Some(*k))
            || !ui.is_displayed(*k)
            || ui.parent(*k) != Some(ui.screen_root())
        {
            continue;
        }
        let d = ui.displayed_box(*k);
        if d.w() <= wu && d.h() <= hu {
            assert!(
                d.l >= 0 && d.t >= 0 && d.r < wu && d.b < hu,
                "I6: root outside F: {d:?} in {wu}x{hu}"
            );
        }
    }
}
