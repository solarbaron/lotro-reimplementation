// SPDX-License-Identifier: GPL-3.0-or-later
//! UI scale (`ui-behaviour.md` §9.2) and screen size changes (§9.4, `ui-layout.md` §8.5).

use tessera_formats::property::PropertyValue;

use crate::events::UiEvent;
use crate::geom::{IBox, add, sub, trunc_f32};
use crate::layout::{RuntimeInput, runtime};
use crate::scaling::{self, ComputedBox, ContentBox, DBox, FitVerticalAnchor};
use crate::tree::ElementKey;
use crate::ui::{Ui, object_mode};

impl Ui {
    /// Scales a root panel by *s* (`ui-behaviour.md` §9.2 steps 2–6). `scaler_type` selects
    /// the resize in object mode Stretch of the game's scaler types (step 6). A hidden
    /// panel takes the scale when it is next shown (`lua-plugin-api.md` §13.21 rule 5).
    pub fn scale_panel(&mut self, k: ElementKey, s: f32, scaler_type: bool) {
        if !s.is_finite() || s <= 0.0 || !self.exists(k) {
            return;
        }
        if !self.is_visible(k) {
            if let Some(e) = self.el_mut(k) {
                e.pending_scale = Some((s, scaler_type));
                e.scaler = e.scaler.or(Some(((e.bx.w(), e.bx.h()), 1.0)));
            }
            return;
        }
        if let Some(e) = self.el_mut(k) {
            e.scaler = Some(e.scaler.unwrap_or(((e.bx.w(), e.bx.h()), 1.0)));
            // A scale applied now supersedes one stored while hidden.
            e.pending_scale = None;
        }
        self.scale_panel_inner(k, s, scaler_type);
    }

    /// Applies a scale stored while the panel was hidden, with the `scaler_type` it was
    /// asked for (`ui-behaviour.md` §9.2 step 6).
    pub(crate) fn apply_panel_scale(&mut self, k: ElementKey, s: f32, scaler_type: bool) {
        self.scale_panel_inner(k, s, scaler_type);
    }

    fn scale_panel_inner(&mut self, k: ElementKey, s: f32, scaler_type: bool) {
        let p = self.v.pids;
        let Some(e) = self.el(k) else { return };
        let ((cw, ch), s0) = e.scaler.unwrap_or(((e.bx.w(), e.bx.h()), 1.0));
        let b = e.bx;
        let (sw, sh) = self.screen_size;
        // Step 2: fit.
        let mut s = s;
        if cw > 0 {
            s = s.min(sw as f32 / cw as f32);
        }
        if ch > 0 {
            s = s.min(sh as f32 / ch as f32);
        }
        // Step 3: requested size, limits scaled by s.
        let mut w = trunc_f32(cw as f32 * s);
        let mut h = trunc_f32(ch as f32 * s);
        let lim = |pid: u32, ui: &Ui| {
            let v = ui.prop(k, pid);
            v.is_set().then(|| trunc_f32(v.int() as f32 * s))
        };
        if let Some(m) = lim(p.min_width, self) {
            w = w.max(m);
        }
        if let Some(m) = lim(p.min_height, self) {
            h = h.max(m);
        }
        if let Some(m) = lim(p.max_width, self) {
            w = w.min(m);
        }
        if let Some(m) = lim(p.max_height, self) {
            h = h.min(m);
        }
        // Step 4: origin kept fixed.
        let ox = self.prop(k, p.scaler_origin_x).int();
        let oy = self.prop(k, p.scaler_origin_y).int();
        let mut left = sub(
            add(b.l, trunc_f32(ox as f32 * s0)),
            trunc_f32(ox as f32 * s),
        );
        let mut top = sub(
            add(b.t, trunc_f32(oy as f32 * s0)),
            trunc_f32(oy as f32 * s),
        );
        // Step 5.
        left = left.min(sub(sw, w)).max(0);
        top = top.min(sub(sh, h)).max(0);
        // Step 6.
        if scaler_type {
            let saved = (
                self.el(k)
                    .and_then(|e| e.runtime.get(&p.object_mode).cloned()),
                self.el(k)
                    .and_then(|e| e.runtime.get(&p.max_width).cloned()),
                self.el(k)
                    .and_then(|e| e.runtime.get(&p.max_height).cloned()),
            );
            let mode = self.object_mode(k);
            if mode != Some(object_mode::STRETCH) {
                self.set_property(k, p.object_mode, PropertyValue::Enum(object_mode::STRETCH));
                self.set_property(k, p.max_width, PropertyValue::Int(w));
                self.set_property(k, p.max_height, PropertyValue::Int(h));
            }
            // Keep the unscaled layout box: the Stretch resize changes the scale vector.
            if let Some(e) = self.el_mut(k) {
                let lb = IBox::from_xywh(e.bx.l, e.bx.t, cw, ch);
                e.bx = lb;
            }
            self.resize(k, w, h);
            if mode != Some(object_mode::STRETCH) {
                for (pid, v) in [p.object_mode, p.max_width, p.max_height]
                    .into_iter()
                    .zip([saved.0, saved.1, saved.2])
                {
                    match v {
                        Some(v) => self.set_property(k, pid, v),
                        None => self.clear_property(k, pid),
                    }
                }
            }
        } else {
            self.resize(k, w, h);
        }
        self.move_to(k, left, top);
        if let Some(e) = self.el_mut(k) {
            e.scaler = Some(((cw, ch), s));
        }
        self.input.hover_dirty = true;
        self.mark_dirty(k);
    }

    /// The scale of a scaled panel (1 when unscaled).
    #[must_use]
    pub fn panel_scale(&self, k: ElementKey) -> f32 {
        self.el(k).and_then(|e| e.scaler).map_or(1.0, |c| c.1)
    }

    /// Sets the display-node scale of an element directly (tests and tools; the panel
    /// scale goes through [`Ui::scale_panel`]).
    pub fn set_node_scale(&mut self, k: ElementKey, scale: Option<[f32; 3]>) {
        if let Some(e) = self.el_mut(k) {
            e.scale = scale;
        }
        self.input.hover_dirty = true;
        self.mark_dirty(k);
    }

    /// Resizes the screen root (§9.4): every root panel is laid out again from its layout's
    /// design size to the new screen size, and game code is notified. A side ≤ 0 gives
    /// 1024 × 768.
    pub fn resize_screen(&mut self, w: i32, h: i32) {
        let (w, h) = if w > 0 && h > 0 {
            (
                w.min(crate::geom::COORD_LIMIT),
                h.min(crate::geom::COORD_LIMIT),
            )
        } else {
            (1024, 768)
        };
        if (w, h) == self.screen_size {
            return;
        }
        self.screen_size = (w, h);
        let root = self.screen;
        if let Some(e) = self.el_mut(root) {
            e.layout_size = (w, h);
        }
        self.set_layout_box(root, IBox::from_xywh(0, 0, w, h));
        self.emit(UiEvent::ScreenResized {
            width: w,
            height: h,
        });
        self.input.hover_dirty = true;
        self.choose_cursor();
    }

    /// Sets the current UI screen *F* and safe area *A* (`ui-scaling.md` §3.2, §5.1), applied
    /// live. The UI host computes `screen` from the window's inner physical size and the
    /// effective scale ([`crate::scaling::effective_scale`], [`crate::scaling::UiScreen::new`])
    /// and `safe` from [`crate::scaling::safe_area`]; this routine resizes the screen root to
    /// *F*'s UI-unit size (`Wu` × `Hu`), so every root panel is laid out again in UI units
    /// (the screen-size path of `ui-layout.md` §8.5 and `ui-behaviour.md` §9.4), and records
    /// the axis factors (`Kx`, `Ky`) used to place every surface on its physical box at
    /// composition (§3.6, §6.6).
    ///
    /// Before any call the UI is at the **identity** (`Se` = 1, *A* = *F* = the screen size
    /// passed to [`Ui::new`]), so existing behaviour and goldens are byte-identical (invariant
    /// I2). At `Se` = 1 the axis factors are 1 and the pixel-centre mapping is `x ↦ x`, so the
    /// identity holds for a real `UiScreen` too (configurations C2, C4, C7, C9, C10, C12).
    ///
    /// A window with a zero side has no screen (`ui-scaling.md` §3.2): the host passes no
    /// `UiScreen` for it ([`crate::scaling::UiScreen::new`] returns `None`) and simply does not
    /// call this, so the last good screen is kept.
    pub fn set_ui_screen(&mut self, screen: crate::scaling::UiScreen, safe: crate::scaling::DBox) {
        self.ui_screen = Some(screen);
        self.safe_area = Some(safe);
        let (wu, hu) = screen.ui_size();
        self.resize_screen(wu, hu);
    }

    /// The current UI screen *F*, or `None` at the identity (`ui-scaling.md` §3.2). The UI
    /// host and tools read it for the pixel-centre mapping of §3.6.
    #[must_use]
    pub fn ui_screen(&self) -> Option<crate::scaling::UiScreen> {
        self.ui_screen
    }

    /// The safe area *A* (`ui-scaling.md` §5.1), in UI units relative to *F*. At the identity
    /// (no UI screen set, or none stored) it is the full screen *F*.
    #[must_use]
    pub fn safe_area_box(&self) -> crate::scaling::DBox {
        if let Some(a) = self.safe_area {
            return a;
        }
        let (w, h) = self.screen_size;
        crate::scaling::DBox::new(0.0, 0.0, f64::from(w), f64::from(h))
    }

    /// Maps a half-open rectangle in UI units (relative to *F*) to its **physical box** in the
    /// window's pixels (`ui-scaling.md` §3.6 "Box → physical"): the physical columns
    /// `Ex(l) … Ex(r) − 1` and rows `Ey(t) … Ey(b) − 1`. At the identity (no UI screen, or
    /// `Se` = 1 so `Kx` = `Ky` = 1) it returns `r` unchanged, which keeps the composition and
    /// hit geometry byte-identical (invariant I2).
    #[must_use]
    pub(crate) fn to_physical_rect(&self, r: crate::paint::Rect) -> crate::paint::Rect {
        match self.ui_screen {
            Some(f) => crate::paint::Rect::new(
                f.edge_x(f64::from(r.x0)),
                f.edge_y(f64::from(r.y0)),
                f.edge_x(f64::from(r.x1)),
                f.edge_y(f64::from(r.y1)),
            ),
            None => r,
        }
    }

    /// The physical box of an element (`ui-scaling.md` §3.6): its displayed box (UI units)
    /// mapped to the window's pixels by [`Ui::to_physical_rect`]. At the identity this equals
    /// the displayed box made a rectangle. Drawing and hit testing agree on it (invariants
    /// I3, I4): a physical pixel lies in this box exactly when a pointer at its centre hits the
    /// displayed box.
    #[must_use]
    pub fn physical_box(&self, k: ElementKey) -> crate::paint::Rect {
        self.to_physical_rect(self.displayed_box(k).to_rect())
    }

    // ----- the live fit rules (`ui-scaling.md` §5.2–§5.6) --------------------------------
    //
    // These run from [`Ui::layout_element`] only while a UI screen *F* is set
    // ([`Ui::set_ui_screen`]) and only for the elements §5 takes over from the plain run-time
    // layout: **fit containers** (§5.4, Types A/B/C) and **safe-area panel roots** (§5.2).
    // Every other element (ordinary children, and full-screen roots with edges (5, 5, 5, 5))
    // keeps the UL §8.4 arithmetic of [`Ui::layout_default`]. At the identity (`Se` = 1 with
    // *A* = *F*) the safe area is the whole screen, the frame offset is 0 and a full-screen
    // root falls through unchanged, so every root lands exactly where it did before scaling was
    // wired in (invariant I2). Fit groups with more than one member, §5.3 per-child safe boxes,
    // the §5.4 letterbox-bar fills (a paint concern) and the anchored saved-placement store are
    // later increments (`ui-scaling.md` §5.9 implementer OPEN).

    /// Lays an element out by the §5 fit rules when they apply, returning `true` when it did
    /// (the caller then skips [`Ui::layout_default`]). Handles fit containers (§5.4) and
    /// safe-area panel roots (§5.2); returns `false` for full-screen roots and ordinary
    /// children, which keep the plain run-time layout.
    pub(crate) fn scaled_layout(&mut self, k: ElementKey) -> bool {
        let (edges, is_root, is_plugin) = match self.el(k) {
            Some(e) => (
                e.edges,
                e.root_panel && e.parent == Some(self.screen),
                e.plugin.is_some(),
            ),
            None => return false,
        };
        if is_plugin {
            return false;
        }
        let l6 = edges[0] == 6;
        let t6 = edges[1] == 6;
        let r6 = edges[2] == 6;
        let b6 = edges[3] == 6;
        let has6 = l6 || t6 || r6 || b6;
        let covers = self.covers_design_area(k);
        let is_stretch = self.object_mode(k) == Some(object_mode::STRETCH);
        // A fit root is a full-cover root with both horizontal edges 6, whatever its object
        // mode (§5.2); any other mode-6 Stretch element is a fit container (§5.4).
        let is_fit_root = is_root && l6 && r6 && covers;
        if (is_stretch && has6) || is_fit_root {
            self.layout_fit_container(k);
            return true;
        }
        if is_root {
            // §5.2: a full-screen root with edges (5, 5, 5, 5) is laid out against *F* by the
            // plain rule (it fills the screen and can never be larger than it); everything else
            // is a safe-area panel, fitted into *A* and then shrunk to the screen if oversize.
            if covers && edges == [5, 5, 5, 5] {
                return false;
            }
            self.layout_root_safe_area(k);
            self.fit_to_screen_root(k);
            return true;
        }
        false
    }

    /// Whether an element's design box covers its layout's design area (`ui-scaling.md` §5.2:
    /// left ≤ 0, top ≤ 0, right ≥ design width − 1, bottom ≥ design height − 1). A layout with a
    /// zero-sided design area (SC1 holds one) has no covering element.
    fn covers_design_area(&self, k: ElementKey) -> bool {
        let Some(e) = self.el(k) else { return false };
        let (dw, dh) = e.layout_size;
        if dw <= 0 || dh <= 0 {
            return false;
        }
        let d = e.design;
        d.l <= 0 && d.t <= 0 && d.r >= dw - 1 && d.b >= dh - 1
    }

    /// Fits a fit container (`ui-scaling.md` §5.4) and applies the result to the live tree: the
    /// layout box keeps its **design size** at the fitted position (in the parent's layout
    /// coordinates), and a **uniform** node scale carries the fit factor so the container's
    /// children — laid out exactly as designed — scale and move with it (§3.4 item 2). Type A
    /// is fitted as a group of one (its own design rectangle is the group box `U`, its direct
    /// children the content box `C`); Types B and C read the box UL §8.4 would give. The pure
    /// arithmetic is [`crate::scaling::fit_type_a`] / `fit_type_b` / `fit_type_c`.
    fn layout_fit_container(&mut self, k: ElementKey) {
        let Some(e) = self.el(k) else { return };
        let edges = e.edges;
        let design = e.design;
        let parent = e.parent;
        let (dw, dh) = (design.w(), design.h());
        if dw < 1 || dh < 1 {
            // Zero design extent: not fitted (§5.4 "Zero extents"); keep the plain layout.
            self.layout_default(k);
            return;
        }
        // The reference box `P` (the parent's current layout size; for a fit root the screen
        // root's size, which is *F*), the parent's displayed top-left and node scale `m`.
        let (p_w, p_h) = match parent {
            Some(p) => {
                let b = self.el(p).map(|pe| pe.bx).unwrap_or_default();
                (b.w(), b.h())
            }
            None => self.screen_size,
        };
        let parent_tl = match parent {
            Some(p) => {
                let d = self.displayed_box(p);
                (f64::from(d.l), f64::from(d.t))
            }
            None => (0.0, 0.0),
        };
        let m = match parent {
            Some(p) => {
                let s = self.node_scale_at_or_above(p);
                (f64::from(s.0), f64::from(s.1))
            }
            None => (1.0, 1.0),
        };
        let (l6, t6, r6, b6) = (edges[0] == 6, edges[1] == 6, edges[2] == 6, edges[3] == 6);
        let fit = if l6 && r6 {
            // Type A (full cover), group of one: U = the container's own design rectangle,
            // C = the union of its direct children, G = P ∩ (A in the parent's coordinates).
            let u = (
                f64::from(design.l),
                f64::from(design.t),
                f64::from(dw),
                f64::from(dh),
            );
            let content = self.type_a_content(k, dw, dh);
            let a_in = scaling::safe_area_in_parent(self.safe_area_box(), parent_tl, m);
            let p_box = DBox::new(0.0, 0.0, f64::from(p_w), f64::from(p_h));
            let g = {
                let i = p_box.intersect(&a_in);
                if i.is_empty() { p_box } else { i }
            };
            let grp =
                scaling::fit_type_a(f64::from(p_w), f64::from(p_h), u, content, g, scaling::RHO);
            // Group of one: the member's design rectangle is U, so no extra offset.
            grp.member(u.0, u.1, u.0, u.1)
        } else {
            // Types B and C read the box UL §8.4 steps 2–5 give in the parent's coordinates.
            let c = self.fit_computed_box(k);
            let computed = ComputedBox {
                left: f64::from(c.l),
                top: f64::from(c.t),
                right: f64::from(c.r),
                bottom: f64::from(c.b),
            };
            if l6 ^ r6 {
                // Type B (one horizontal edge 6): the height drives; left edge 6 → anchored at
                // the computed right, right edge 6 → at the computed left.
                scaling::fit_type_b(
                    f64::from(c.h()),
                    f64::from(p_w),
                    f64::from(dw),
                    f64::from(dh),
                    computed,
                    l6,
                )
            } else {
                // Type C (a vertical edge 6, no horizontal one): the width drives.
                let anchor = match (t6, b6) {
                    (true, true) => FitVerticalAnchor::Centre,
                    (true, false) => FitVerticalAnchor::Bottom,
                    _ => FitVerticalAnchor::Top,
                };
                scaling::fit_type_c(
                    f64::from(c.w()),
                    f64::from(p_h),
                    f64::from(dw),
                    f64::from(dh),
                    computed,
                    anchor,
                )
            }
        };
        let f = fit.f as f32;
        let fx = round_i32(fit.x);
        let fy = round_i32(fit.y);
        if let Some(e) = self.el_mut(k) {
            e.scale = if (f - 1.0).abs() < 1e-6 {
                None
            } else {
                Some([f, f, 1.0])
            };
        }
        self.set_layout_box(k, IBox::from_xywh(fx, fy, dw, dh));
        self.mark_dirty(k);
        self.input.hover_dirty = true;
    }

    /// The content box `C` of a group-of-one Type A fit container (`ui-scaling.md` §5.4): the
    /// smallest box holding its direct children of extent ≥ 1, each cut to the container's
    /// design rectangle, relative to the group box `U`'s top-left (= the container's design
    /// top-left, so children — given relative to the container — are already in that frame).
    /// `None` when no such child exists.
    fn type_a_content(&self, k: ElementKey, dw: i32, dh: i32) -> Option<ContentBox> {
        let mut l = f64::INFINITY;
        let mut t = f64::INFINITY;
        let mut r = f64::NEG_INFINITY;
        let mut b = f64::NEG_INFINITY;
        for c in self.children(k) {
            let Some(e) = self.el(c) else { continue };
            let d = e.design;
            if d.w() < 1 || d.h() < 1 {
                continue;
            }
            let cl = d.l.max(0);
            let ct = d.t.max(0);
            let cr = add(d.l, d.w()).min(dw);
            let cb = add(d.t, d.h()).min(dh);
            if cr <= cl || cb <= ct {
                continue;
            }
            l = l.min(f64::from(cl));
            t = t.min(f64::from(ct));
            r = r.max(f64::from(cr));
            b = b.max(f64::from(cb));
        }
        if !l.is_finite() {
            return None;
        }
        Some(ContentBox {
            x: l,
            y: t,
            w: r - l,
            h: b - t,
        })
    }

    /// The box UL §8.4 steps 2–5 give for an element in its parent's current box, without
    /// setting it (`ui-scaling.md` §5.4 Types B and C read this to drive their one axis). It is
    /// the plain run-time layout of [`Ui::layout_default`] computed but not applied.
    fn fit_computed_box(&self, k: ElementKey) -> IBox {
        let (old, new) = self.reference_boxes(k);
        let Some(e) = self.el(k) else {
            return IBox::default();
        };
        let inp = RuntimeInput {
            design: e.design,
            current: e.bx,
            edges: e.edges,
            old,
            new,
            finished: e.created,
            keep_inside: e.flags & 0x02 != 0,
        };
        runtime(&inp)
    }

    /// Lays a safe-area panel root out in the frame of the safe area *A* (`ui-scaling.md` §5.2):
    /// the UL §8.4 arithmetic runs with *N* = (0, 0, *Aw*, *Hu*), and the result is moved right
    /// by *Ax*. The frame offset last added is recorded and subtracted first (§5.2 "Layout
    /// frames"), so a mode-0 (or dragged) edge keeps its distance from the frame's left across
    /// re-layouts and a round trip of *A* restores the box exactly. At the identity (*A* = *F*)
    /// *Aw* = *Wu*, *Ax* = 0 and this equals the plain layout against *F*.
    fn layout_root_safe_area(&mut self, k: ElementKey) {
        let a = self.safe_area_box();
        let ax = round_i32(a.l);
        let aw = round_i32(a.w);
        let hu = self.screen_size.1;
        let prev = self.frame_offsets.get(&k).copied().unwrap_or((0, 0));
        let Some(e) = self.el(k) else { return };
        let base = e.bx.offset(-prev.0, -prev.1);
        let o = IBox::from_xywh(0, 0, e.layout_size.0, e.layout_size.1);
        let n = IBox::from_xywh(0, 0, aw, hu);
        let inp = RuntimeInput {
            design: e.design,
            current: base,
            edges: e.edges,
            old: o,
            new: n,
            finished: e.created,
            keep_inside: e.flags & 0x02 != 0,
        };
        let b = runtime(&inp).offset(ax, 0);
        self.frame_offsets.insert(k, (ax, 0));
        self.set_layout_box(k, b);
    }

    /// Shrinks a root panel that came out larger than the screen so it fits (`ui-scaling.md`
    /// §5.6): with the fit-to-screen factor `q` = min(1, *Wu* ÷ width, *Hu* ÷ height) applied as
    /// a uniform node scale. Only roots without an existing node scale are touched this
    /// increment (a per-window or plugin `SetScale` factor, and a fit root's own factor, are
    /// left as they are); `q` = 1 (the panel already fits) changes nothing.
    fn fit_to_screen_root(&mut self, k: ElementKey) {
        let b = match self.el(k) {
            Some(e) if e.scale.is_none() => e.bx,
            _ => return,
        };
        let (wu, hu) = self.screen_size;
        let q = scaling::fit_to_screen(f64::from(b.w()), f64::from(b.h()), wu, hu);
        if q < 1.0 {
            let q = q as f32;
            if let Some(e) = self.el_mut(k) {
                e.scale = Some([q, q, 1.0]);
            }
            self.mark_dirty(k);
            self.input.hover_dirty = true;
        }
    }
}

/// Rounds a real UI coordinate to the nearest integer layout unit, halves up (`ui-scaling.md`
/// §3.5 "Moves and resizes"). A non-finite value becomes 0; the result is clamped into `i32`.
fn round_i32(v: f64) -> i32 {
    if !v.is_finite() {
        return 0;
    }
    (v + 0.5)
        .floor()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
}
