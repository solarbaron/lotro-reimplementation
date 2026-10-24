// SPDX-License-Identifier: GPL-3.0-or-later
//! UI scale (`ui-behaviour.md` §9.2) and screen size changes (§9.4, `ui-layout.md` §8.5).

use tessera_formats::property::PropertyValue;

use crate::events::UiEvent;
use crate::geom::{IBox, add, sub, trunc_f32};
use crate::layout::{RuntimeInput, runtime};
use crate::scaling::{self, ComputedBox, ContentBox, DBox, FitMember, FitVerticalAnchor};
use crate::tree::ElementKey;
use crate::ui::{MAX_TREE_DEPTH, Ui, object_mode};

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
        let was_inside = self.root_inside_screen(k);
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
            // Back at scale 1 the rule no longer owns the node scale: the panel may take a
            // per-window factor again (`ui-scaling.md` §7.4, [`Ui::accepts_window_scale`]).
            e.scaler = ((s - 1.0).abs() > f32::EPSILON).then_some(((cw, ch), s));
        }
        // `ui-scaling.md` §5.6 and §5.8 item 3: a per-window scale change recomputes the
        // fit-to-screen factor and keeps the panel on screen (under a UI screen only).
        self.after_scale_change(k, was_inside);
        // §3.4 item 2: the fit containers inside follow the panel's new scale and position.
        self.refit_descendants(k);
        self.input.hover_dirty = true;
        self.mark_dirty(k);
    }

    /// Sets the **per-window scale** *p* of a native root panel (`ui-scaling.md` §7.4): a
    /// uniform node scale (§3.4), so the panel's layout box keeps its size and its children are
    /// not laid out again, and its content is painted at *Se* × *p*. *p* multiplies the
    /// panel's other node scales (§3.4: *m* is their product): a Stretch root keeps its stretch
    /// vector under *p*, and §5.6's *q* multiplies both.
    ///
    /// The scaling origin (`UIComponent_UIScaler_Origin_X`/`_Y`, unscaled panel pixels,
    /// `ui-behaviour.md` §9.2 step 4) stays in place in UI units. The origin's position is kept
    /// unrounded from the moment *p* leaves 1 (and taken again from the panel's current
    /// position after anything else moved it), and the layout position is that point minus
    /// origin × *p*, rounded once (halves up); so steps do not add up rounding errors and a
    /// round trip returns the panel to where it started. Then §5.6 (*q* multiplies *p*) and
    /// keep-on-screen run (§5.8 item 3), and the fit containers inside are fitted again.
    ///
    /// The caller clamps *p* to the group's range (§7.4). Returns `false` and changes nothing
    /// for a non-positive or non-finite *p* and for an element that
    /// [`Ui::accepts_window_scale`] refuses.
    pub fn set_window_scale(&mut self, k: ElementKey, p: f32) -> bool {
        if !p.is_finite() || p <= 0.0 || !self.accepts_window_scale(k) {
            return false;
        }
        let p0 = self.window_scale(k);
        if (p - p0).abs() <= f32::EPSILON {
            return true;
        }
        let was_inside = self.root_inside_screen(k);
        let pids = self.v.pids;
        let ox = f64::from(self.prop(k, pids.scaler_origin_x).int());
        let oy = f64::from(self.prop(k, pids.scaler_origin_y).int());
        let Some((b, cur)) = self.el(k).map(|e| (e.bx, e.scale)) else {
            return false;
        };
        // The panel's own scale without q (§5.6 keeps the scale it multiplied) and without the
        // old p: a stretch vector, or 1.
        let base = match self.fit_q.get(&k) {
            Some(r) if r.written == cur => r.base,
            _ => cur,
        };
        let own = base.map_or([1.0, 1.0, 1.0], |s| [s[0] / p0, s[1] / p0, s[2]]);
        let new = [own[0] * p, own[1] * p, own[2]];
        let identity = new.iter().all(|v| (v - 1.0).abs() <= f32::EPSILON);
        let (pf, p0f) = (f64::from(p), f64::from(p0));
        let (ax, ay) = match self.window_anchor.get(&k) {
            Some(a) if a.at == (b.l, b.t) => (a.x, a.y),
            _ => (f64::from(b.l) + ox * p0f, f64::from(b.t) + oy * p0f),
        };
        let left = round_i32(ax - ox * pf);
        let top = round_i32(ay - oy * pf);
        let unit = (p - 1.0).abs() <= f32::EPSILON;
        if unit {
            self.window_p.remove(&k);
        } else {
            self.window_p.insert(k, p);
        }
        if let Some(e) = self.el_mut(k) {
            e.scale = (!identity).then_some(new);
        }
        self.painter.note_structure();
        self.move_to(k, left, top);
        self.after_scale_change(k, was_inside);
        self.refit_descendants(k);
        self.input.hover_dirty = true;
        self.mark_dirty(k);
        // Keep the origin's real position for the next step, unless keep-on-screen moved the
        // panel (then the new position is the start of the next step).
        if unit {
            self.window_anchor.remove(&k);
        } else if let Some(now) = self.el(k).map(|e| (e.bx.l, e.bx.t)) {
            let a = if now == (left, top) {
                WindowAnchor {
                    x: ax,
                    y: ay,
                    at: now,
                }
            } else {
                WindowAnchor {
                    x: f64::from(now.0) + ox * pf,
                    y: f64::from(now.1) + oy * pf,
                    at: now,
                }
            };
            self.window_anchor.insert(k, a);
        }
        true
    }

    /// The per-window scale *p* of a root panel (`ui-scaling.md` §7.4): 1 when none was set.
    /// It excludes the fit-to-screen factor *q* that may multiply it (§5.6).
    #[must_use]
    pub fn window_scale(&self, k: ElementKey) -> f32 {
        self.window_p.get(&k).copied().unwrap_or(1.0)
    }

    /// Whether [`Ui::set_window_scale`] applies to `k`: a native (not plugin) root panel laid
    /// out against the screen, which is no fit container or fit root (their node scale is
    /// their fit factor) and is not scaled by the panel-scale rule of `ui-behaviour.md` §9.2
    /// ([`Ui::scale_panel`] at a scale other than 1), whose node scale that rule owns. The UI
    /// host asks this before it changes a group's factor for the panel under the pointer.
    #[must_use]
    pub fn accepts_window_scale(&self, k: ElementKey) -> bool {
        self.el(k)
            .is_some_and(|e| e.plugin.is_none() && e.scaler.is_none())
            && self.is_screen_root(k)
            && !self.is_fit_root(k)
            && self.fit_kind(k).is_none()
    }

    /// The root panel laid out against the screen (`ui-scaling.md` §5.2) that holds `k`, or
    /// `k` itself when it is one; `None` for the screen root and detached elements. The UI
    /// host finds the panel under the pointer with it (the per-window scale gesture, §7.4).
    #[must_use]
    pub fn screen_root_of(&self, k: ElementKey) -> Option<ElementKey> {
        let mut cur = Some(k);
        let mut n = 0;
        while let Some(c) = cur {
            if self.is_screen_root(c) {
                return Some(c);
            }
            cur = self.parent(c);
            n += 1;
            if n > crate::ui::MAX_TREE_DEPTH * 4 {
                break;
            }
        }
        None
    }

    /// The scale of a scaled panel (1 when unscaled).
    #[must_use]
    pub fn panel_scale(&self, k: ElementKey) -> f32 {
        self.el(k).and_then(|e| e.scaler).map_or(1.0, |c| c.1)
    }

    /// Sets the display-node scale of an element directly (tests and tools; the panel
    /// scale goes through [`Ui::scale_panel`]).
    ///
    /// Under a UI screen a root panel's node scale is its own (per-window) scale: the
    /// fit-to-screen factor *q* of `ui-scaling.md` §5.6 multiplies it, and keep-on-screen runs
    /// (§5.8 item 3). The fit containers inside the element are fitted again (§3.4 item 2:
    /// their *m*(parent) changed).
    pub fn set_node_scale(&mut self, k: ElementKey, scale: Option<[f32; 3]>) {
        let was_inside = self.root_inside_screen(k);
        if let Some(e) = self.el_mut(k) {
            e.scale = scale;
        }
        self.after_scale_change(k, was_inside);
        self.refit_descendants(k);
        self.input.hover_dirty = true;
        // A surface owner's canvas follows its geometry (`ui-scaling.md` §6.1): no content of
        // its own changes.
        self.mark_geometry_dirty(k);
    }

    /// Resizes the screen root (§9.4): every root panel is laid out again from its layout's
    /// design size to the new screen size, and game code is notified. A side ≤ 0 gives
    /// 1024 × 768 at the identity; under a UI screen (`ui-scaling.md` §3.2 "A window with a
    /// zero side") a side ≤ 0 changes nothing, because laying out at a fallback size would move
    /// native panels and plugin windows for good.
    pub fn resize_screen(&mut self, w: i32, h: i32) {
        if self.ui_screen.is_some() && (w <= 0 || h <= 0) {
            return;
        }
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
    /// and `safe` from [`crate::scaling::safe_area()`]; this routine resizes the screen root to
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
    ///
    /// After the layout (§4.5 step 2, §5.8 items 1, 3 and 5) saved placements are applied again
    /// when *F* or *A* changed (their position and size; the visibility a placement carries is
    /// applied only when the panel is opened) and keep-on-screen runs for every native root
    /// panel (fit roots excepted), and a plugin
    /// window that lay entirely inside the old *F* is kept inside the new one; a plugin window
    /// the plugin placed partly off screen stays where it is. A change of the safe area alone
    /// (a new `ui_safe_aspect` at the same UI-screen size) lays the roots out again too. A call
    /// with the screen and safe area already in force changes nothing (a window restored to its
    /// size after a minimise moves nothing, invariant I1). `safe` is clamped into *F*; a
    /// non-finite one is taken as *F*.
    pub fn set_ui_screen(&mut self, screen: crate::scaling::UiScreen, safe: crate::scaling::DBox) {
        let (wu, hu) = screen.ui_size();
        let safe = sanitize_safe(safe, wu, hu);
        let first = self.ui_screen.is_none();
        let old_f = self.screen_size;
        let old_a = self.safe_area_box();
        let old_screen = self.ui_screen;
        // §5.8 item 5: the plugin windows that lie entirely inside the old F.
        let inside: Vec<ElementKey> = self
            .children(self.screen)
            .into_iter()
            .filter(|r| {
                self.el(*r)
                    .is_some_and(|e| e.root_panel && e.plugin.is_some())
                    && self.root_inside_screen(*r)
            })
            .collect();
        self.ui_screen = Some(screen);
        self.safe_area = Some(safe);
        let f_changed = (wu, hu) != old_f;
        let a_changed = safe != old_a;
        let se_changed = old_screen != Some(screen);
        if old_screen.map(|f| f.effective_scale()) != Some(screen.effective_scale()) {
            // §8 S6: the memory split follows Se; the glyph caches evict when it falls.
            self.apply_s6();
        }
        if f_changed {
            self.resize_screen(wu, hu);
        }
        if first || f_changed || a_changed {
            self.relayout_for_screen_change(first, f_changed);
        }
        if first || f_changed || a_changed || se_changed {
            // §5.8 "Applying": saved placements are applied again after a change of F or A
            // only; a change of Se alone runs §5.6 and keep-on-screen (§5.8 item 3).
            self.after_screen_change(&inside, first || f_changed || a_changed);
        }
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

    /// The physical box of an element (`ui-scaling.md` §3.6 "Box → physical"): its **real**
    /// displayed box [*L*, *R*) × [*T*, *B*) (§3.4, [`Ui::displayed_real`]) mapped edge by edge,
    /// columns `Ex(L) … Ex(R) − 1` and rows `Ey(T) … Ey(B) − 1`, with no rounding to whole UI
    /// units first. At the identity this equals the displayed box made a rectangle. Drawing and
    /// hit testing agree on it (invariants I3, I4): a physical pixel lies in this box exactly
    /// when a pointer at its centre hits the real displayed box ([`Ui::hit_test_real`]).
    #[must_use]
    pub fn physical_box(&self, k: ElementKey) -> crate::paint::Rect {
        match self.ui_screen {
            Some(_) => self.real_to_physical(&self.real_edges(k).0),
            None => self.displayed_box(k).to_rect(),
        }
    }

    /// Maps real displayed edges (UI units) to physical pixels by §3.6 "Box → physical" (an
    /// extent that maps to no pixel gives an empty rectangle). At the identity the edges are
    /// truncated (they are whole numbers there).
    pub(crate) fn real_to_physical(&self, b: &crate::ui::RealBox) -> crate::paint::Rect {
        match self.ui_screen {
            Some(f) => {
                crate::paint::Rect::new(f.edge_x(b.l), f.edge_y(b.t), f.edge_x(b.r), f.edge_y(b.b))
            }
            None => crate::paint::Rect::new(
                crate::geom::trunc_f64(b.l),
                crate::geom::trunc_f64(b.t),
                crate::geom::trunc_f64(b.r),
                crate::geom::trunc_f64(b.b),
            ),
        }
    }

    // ----- the live fit rules (`ui-scaling.md` §5.2–§5.8) --------------------------------
    //
    // These run from [`Ui::layout_element`] only while a UI screen *F* is set
    // ([`Ui::set_ui_screen`]) and only for the elements §5 takes over from the plain run-time
    // layout: **fit containers** (§5.4, Types A/B/C, Type A in fit groups), **safe-area panel
    // roots** (§5.2; modal dialogs included, laid out in their container's safe box) and the
    // **direct children of full-screen (5, 5, 5, 5) roots** that are not backdrops (§5.3, laid
    // out in the root's safe box). Every other element (ordinary children, backdrops and
    // full-screen roots with edges (5, 5, 5, 5)) keeps the UL §8.4 arithmetic of
    // [`Ui::layout_default`]. At the identity (`Se` = 1 with *A* = *F*) the safe area is the
    // whole screen, every frame offset is 0 and the safe box of a full-screen root is its whole
    // box, so every root and child lands exactly where the plain layout puts it; only fit
    // containers change, by design (§9.1).

    /// Lays an element out by the §5 fit rules when they apply, returning `true` when it did
    /// (the caller then skips [`Ui::layout_default`]). Handles fit containers (§5.4), safe-area
    /// panel roots (§5.2, with §5.6 and keep-on-screen, §5.8 item 1) and the non-backdrop
    /// children of full-screen roots (§5.3); returns `false` for full-screen roots, backdrops
    /// and ordinary children, which keep the plain run-time layout.
    pub(crate) fn scaled_layout(&mut self, k: ElementKey) -> bool {
        let Some(e) = self.el(k) else { return false };
        if e.plugin.is_some() {
            return false;
        }
        let is_root = self.is_screen_root(k);
        if let Some(kind) = self.fit_kind(k) {
            let fit_root = self.is_fit_root(k);
            match kind {
                FitKind::A => self.layout_type_a(k),
                FitKind::B | FitKind::C => {
                    // §5.2: a root that is not full-screen is a safe-area panel, laid out in the
                    // frame of A (§5.4 Type B: "in the box it is laid out in"); a non-backdrop
                    // child of a full-screen root is laid out in the root's safe box (§5.3).
                    let frame = if is_root {
                        self.safe_area_frame(k)
                    } else {
                        self.safe_box_frame(k)
                    };
                    self.layout_type_bc(k, kind, frame);
                }
            }
            if is_root && !fit_root {
                // A fit container that is a safe-area panel root (not a fit root) gets §5.6 and
                // keep-on-screen like any other safe-area root (§5.2, §5.8 item 1).
                self.refresh_fit_to_screen(k);
                self.keep_root_on_screen(k);
            }
            return true;
        }
        if is_root {
            // §5.2: a full-screen root with edges (5, 5, 5, 5) is laid out against *F* by the
            // plain rule (it fills the screen and can never be larger than it); everything else
            // is a safe-area panel, fitted into *A*, shrunk to the screen if oversize (§5.6) and
            // kept on screen (§5.8 item 1).
            if self.is_full_screen_root(k) {
                return false;
            }
            if let Some(frame) = self.safe_area_frame(k) {
                self.layout_in_frame(k, &frame);
            }
            self.refresh_fit_to_screen(k);
            self.keep_root_on_screen(k);
            return true;
        }
        if let Some(frame) = self.safe_box_frame(k) {
            self.layout_in_frame(k, &frame);
            return true;
        }
        false
    }

    /// Whether `k` is a root panel laid out against the screen (`ui-scaling.md` §5.2): attached
    /// to the screen root, or a modal dialog attached to its dialog container, which is itself
    /// attached to the screen root (the container fills *F*, at the origin and without a node
    /// scale, so the dialog's layout coordinates are UI units relative to *F*).
    pub(crate) fn is_screen_root(&self, k: ElementKey) -> bool {
        let Some(e) = self.el(k) else { return false };
        if !e.root_panel {
            return false;
        }
        match e.parent {
            Some(p) if p == self.screen => true,
            Some(p) => self
                .el(p)
                .is_some_and(|c| c.dialog_container && c.parent == Some(self.screen)),
            None => false,
        }
    }

    /// The root panels laid out against the screen (`ui-scaling.md` §5.2), in drawing order:
    /// the screen root's root children and the modal dialogs inside its dialog containers.
    /// The UI host gives every one of them with a scaling group its group's per-window factor
    /// (§7.4).
    #[must_use]
    pub fn screen_roots(&self) -> Vec<ElementKey> {
        self.layout_roots()
    }

    /// The root panels laid out against the screen, in drawing order: the screen root's root
    /// children and the modal dialogs inside its dialog containers (`ui-scaling.md` §5.2).
    pub(crate) fn layout_roots(&self) -> Vec<ElementKey> {
        let mut out = Vec::new();
        for c in self.children(self.screen) {
            let Some(e) = self.el(c) else { continue };
            if e.root_panel {
                out.push(c);
            } else if e.dialog_container {
                out.extend(
                    e.children
                        .iter()
                        .copied()
                        .filter(|d| self.el(*d).is_some_and(|de| de.root_panel)),
                );
            }
        }
        out
    }

    /// Whether `k` is a **fit root** (`ui-scaling.md` §5.2): a native root panel attached to the
    /// screen root whose design box covers its layout's design area and whose left and right
    /// edges are both 6. Fit roots are Type A fit containers placed against *F*, exempt from
    /// §5.6, keep-on-screen and saved placements, and fitted with the other fit roots of their
    /// layout as one fit group. Only meaningful while a UI screen is set.
    pub(crate) fn is_fit_root(&self, k: ElementKey) -> bool {
        let Some(e) = self.el(k) else { return false };
        e.plugin.is_none()
            && e.root_panel
            && e.parent == Some(self.screen)
            && e.edges[0] == 6
            && e.edges[2] == 6
            && self.covers_design_area(k)
    }

    /// Whether `k` is a full-screen root with edges (5, 5, 5, 5) (`ui-scaling.md` §5.2): its
    /// direct children are laid out by §5.3. A modal dialog that covers its layout's design area
    /// with these edges is one too (it is a root panel laid out against the screen).
    fn is_full_screen_root(&self, k: ElementKey) -> bool {
        self.el(k)
            .is_some_and(|e| e.plugin.is_none() && e.edges == [5, 5, 5, 5])
            && self.is_screen_root(k)
            && self.covers_design_area(k)
    }

    /// The fit-container type of an element (`ui-scaling.md` §5.4), or `None` when it is not a
    /// fit container: a fit root is Type A; otherwise an element in object mode Stretch with an
    /// edge in mode 6 is Type A (both horizontal edges 6), B (exactly one) or C (none, a vertical
    /// one).
    fn fit_kind(&self, k: ElementKey) -> Option<FitKind> {
        let e = self.el(k)?;
        if e.plugin.is_some() {
            return None;
        }
        if self.is_fit_root(k) {
            return Some(FitKind::A);
        }
        let [l, t, r, b] = e.edges.map(|m| m == 6);
        if !(l || t || r || b) || self.object_mode(k) != Some(object_mode::STRETCH) {
            return None;
        }
        Some(if l && r {
            FitKind::A
        } else if l ^ r {
            FitKind::B
        } else {
            FitKind::C
        })
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

    /// The layout frame of a root panel that is a safe-area panel (`ui-scaling.md` §5.2): *O* =
    /// its layout's design area, *N* = (0, 0, *Aw*, *Hu*), moved right by *Ax*. For a modal
    /// dialog this is its container's safe box (the container fills *F*, so the part of it
    /// inside *A* is *A*). At the identity (*A* = *F*) it is the plain screen-level layout.
    fn safe_area_frame(&self, k: ElementKey) -> Option<Frame> {
        let e = self.el(k)?;
        let a = self.safe_area_box();
        Some(Frame {
            old: IBox::from_xywh(0, 0, e.layout_size.0, e.layout_size.1),
            new: IBox::from_xywh(0, 0, round_i32(a.w), self.screen_size.1),
            offset: (round_i32(a.l), 0),
        })
    }

    /// The §5.3 layout frame of a direct child of a full-screen (5, 5, 5, 5) root that is not a
    /// backdrop (`ui-scaling.md` §5.3): *O* = the root's design box (or the child's code
    /// reference box), *N* = the size of the root's **safe box** (the part of the root's current
    /// box inside *A*, or the whole box when that part is empty) and the safe box's offset inside
    /// the root. `None` for every other element (backdrops are laid out against the full box by
    /// the plain rule).
    fn safe_box_frame(&self, k: ElementKey) -> Option<Frame> {
        let e = self.el(k)?;
        let p = e.parent?;
        if !self.is_full_screen_root(p) {
            return None;
        }
        let pe = self.el(p)?;
        let pd = pe.design;
        let d = e.design;
        let backdrop = (d.l <= 0 && d.t <= 0 && d.r >= pd.w() - 1 && d.b >= pd.h() - 1)
            || (e.edges[0] == 6 && e.edges[2] == 6);
        if backdrop {
            return None;
        }
        // The root's box in UI units relative to F: a full-screen root's parent (the screen
        // root, or a dialog container filling F) is at the origin with no node scale.
        let rb = pe.bx;
        let a = self.safe_area_box();
        let (ax0, ax1) = (round_i32(a.l), round_i32(a.right()));
        let (ay0, ay1) = (round_i32(a.t), round_i32(a.bottom()));
        let sl = rb.l.max(ax0);
        let sr = add(rb.r, 1).min(ax1);
        let st = rb.t.max(ay0);
        let sb = add(rb.b, 1).min(ay1);
        let (offset, new) = if sr > sl && sb > st {
            (
                (sub(sl, rb.l), sub(st, rb.t)),
                IBox::from_xywh(0, 0, sub(sr, sl), sub(sb, st)),
            )
        } else {
            ((0, 0), IBox::from_xywh(0, 0, rb.w(), rb.h()))
        };
        Some(Frame {
            old: e.ref_box.unwrap_or(pd),
            new,
            offset,
        })
    }

    /// Lays an element out by UL §8.4 steps 2–5 in a layout frame and moves it by the frame's
    /// offset (`ui-scaling.md` §5.2 "Layout frames"): the offset recorded at its last layout is
    /// subtracted from its current box first, so mode-0 edges (and user or keep-on-screen moves)
    /// keep their distance from the frame's left across any number of re-layouts.
    fn layout_in_frame(&mut self, k: ElementKey, frame: &Frame) {
        let b = self.frame_box(k, frame);
        self.record_frame_offset(k, frame.offset);
        self.set_layout_box(k, b);
    }

    /// The box UL §8.4 steps 2–5 give in `frame` (with the recorded offset taken out of the
    /// current box first), moved by the frame's current offset. Not applied.
    fn frame_box(&self, k: ElementKey, frame: &Frame) -> IBox {
        let prev = self.frame_offsets.get(&k).copied().unwrap_or((0, 0));
        let Some(e) = self.el(k) else {
            return IBox::default();
        };
        let inp = RuntimeInput {
            design: e.design,
            current: e.bx.offset(-prev.0, -prev.1),
            edges: e.edges,
            old: frame.old,
            new: frame.new,
            finished: e.created,
            keep_inside: e.flags & 0x02 != 0,
        };
        runtime(&inp).offset(frame.offset.0, frame.offset.1)
    }

    /// Records the frame offset added at an element's last layout (an offset of 0 is not kept).
    fn record_frame_offset(&mut self, k: ElementKey, offset: (i32, i32)) {
        if offset == (0, 0) {
            self.frame_offsets.remove(&k);
        } else {
            self.frame_offsets.insert(k, offset);
        }
    }

    /// The plain (or, in a frame, framed) layout of a fit container that is **not fitted**
    /// (`ui-scaling.md` §5.4 "Zero extents"): fit factor 1 (no node scale of its own, so it is
    /// displayed through *m*(parent)) and the box UL §8.4 steps 2–5 and the guard of §5.7
    /// compute.
    fn layout_not_fitted(&mut self, k: ElementKey, frame: Option<&Frame>) {
        let m = self.parent_scale(k);
        if self.set_fit_scale(k, 1.0, m) {
            self.refit_descendants(k);
        }
        match frame {
            Some(fr) => self.layout_in_frame(k, fr),
            None => self.layout_default(k),
        }
    }

    /// `m`(parent) of an element (`ui-scaling.md` §3.4): the node scale at or above its parent,
    /// (1, 1, 1) for a child of the screen root or of nothing.
    fn parent_scale(&self, k: ElementKey) -> (f32, f32, f32) {
        match self.parent(k) {
            Some(p) if p != self.screen => self.node_scale_at_or_above(p),
            _ => (1.0, 1.0, 1.0),
        }
    }

    /// Sets a fit container's node scale to *f* × *m*(parent) (`ui-scaling.md` §3.4 item 2:
    /// *m*(*e*) = *f* × *m*(*P*), so the container's displayed size is *w* × *f* × *m*(*P*) and
    /// its content scale *Se* × *f* × *m*(*P*)). The tree reads only the nearest node scale
    /// (UB46), so the product is stored; the bare factor *f* is kept apart for
    /// [`Ui::fit_factor`]. A factor of 1 stores no scale (the container then shows through
    /// *m*(parent) as it is). Returns whether the stored scale changed.
    fn set_fit_scale(&mut self, k: ElementKey, f: f64, m: (f32, f32, f32)) -> bool {
        let f = f as f32;
        let unit = !f.is_finite() || f <= 0.0 || (f - 1.0).abs() < 1e-6;
        if unit {
            self.fit_f.remove(&k);
        } else {
            self.fit_f.insert(k, f);
        }
        let scale = if unit {
            None
        } else {
            Some([f * m.0, f * m.1, m.2])
        };
        let changed = match self.el_mut(k) {
            Some(e) if e.scale != scale => {
                e.scale = scale;
                true
            }
            _ => false,
        };
        if changed {
            self.painter.note_structure();
            self.mark_dirty(k);
            self.input.hover_dirty = true;
        }
        changed
    }

    /// Applies a fit to the live tree (`ui-scaling.md` §3.4 item 2): the container's layout box
    /// keeps its **design size** at the fitted position (in its parent's layout coordinates,
    /// rounded to the nearest unit, halves up) and a **uniform** node scale *f* × *m*(parent)
    /// carries the fit factor, so its children — laid out exactly as designed — scale and move
    /// with it. When the scale or the box changed, the fit containers inside it are fitted
    /// again, because their *m*(parent) and their parents' displayed boxes moved with it
    /// (§5.4 "Nesting and fitting order").
    fn apply_fit(&mut self, k: ElementKey, fit: scaling::Fit, dw: i32, dh: i32) {
        let m = self.parent_scale(k);
        let scale_changed = self.set_fit_scale(k, fit.f, m);
        let b = IBox::from_xywh(round_i32(fit.x), round_i32(fit.y), dw, dh).clamped();
        let box_changed = self.el(k).is_some_and(|e| e.bx != b);
        // §3.4 item 2: the displayed box starts from the real fitted position, not the
        // rounded layout position (`Ui::displayed_real`).
        let old_pos = self.fit_pos.insert(k, (fit.x, fit.y));
        let pos_changed = old_pos != Some((fit.x, fit.y));
        if pos_changed {
            self.painter.note_structure();
        }
        self.set_layout_box(k, b);
        self.mark_dirty(k);
        self.input.hover_dirty = true;
        if scale_changed || box_changed || pos_changed {
            self.refit_descendants(k);
        }
    }

    /// Fits every fit container strictly inside `k` again, parents first (`ui-scaling.md` §5.4
    /// "Nesting and fitting order"): after a change of `k`'s node scale (a per-window, plugin
    /// `SetScale`, fit-to-screen or outer fit factor) or of its displayed position, the inner
    /// containers' *m*(parent) and their guard boxes (*A* taken into their parents'
    /// coordinates) changed although no parent changed size. Only under a UI screen; bounded.
    pub(crate) fn refit_descendants(&mut self, k: ElementKey) {
        if self.ui_screen.is_none() {
            return;
        }
        let mut stack: Vec<ElementKey> = self.children(k).into_iter().rev().collect();
        let mut n = 0usize;
        while let Some(c) = stack.pop() {
            n += 1;
            if n > crate::ui::MAX_ELEMENTS {
                break;
            }
            if self.fit_kind(c).is_some() {
                self.layout_element(c);
            }
            if let Some(e) = self.el(c) {
                stack.extend(e.children.iter().rev().copied());
            }
        }
    }

    /// The parent's displayed top-left and node scale `m(parent)` of a fit container (for a
    /// fit root, the screen root's: the origin and 1).
    fn parent_frame(&self, parent: Option<ElementKey>) -> ((f64, f64), (f64, f64)) {
        match parent {
            Some(p) if p != self.screen => {
                // The real displayed top-left of §3.4, not its rounded integer form.
                let d = self.displayed_real(p);
                let s = self.node_scale_at_or_above(p);
                ((d.l, d.t), (f64::from(s.0), f64::from(s.1)))
            }
            _ => ((0.0, 0.0), (1.0, 1.0)),
        }
    }

    /// Fits the Type A **fit group** of `k` (`ui-scaling.md` §5.4 "Fit groups", steps 1–5) and
    /// applies the one factor and the one map from *P*'s design coordinates to every member,
    /// hidden members included, so layers of one picture stay in register and showing or hiding
    /// one never moves another (invariant I16). The group box *U* is the union of the members'
    /// design rectangles and the content box *C* the union of their direct children (cut to each
    /// member, taken into *P*'s design coordinates). *P* is *F* for a fit root, the safe area's
    /// frame for a Type A root that is a safe-area panel (§5.2; a group of one) and the parent's
    /// box otherwise.
    fn layout_type_a(&mut self, k: ElementKey) {
        let Some(e) = self.el(k) else { return };
        let design = e.design;
        let parent = e.parent;
        let fit_root = self.is_fit_root(k);
        let safe_root = !fit_root && self.is_screen_root(k);
        if design.w() < 1 || design.h() < 1 {
            // Zero design extent: not fitted, takes no part in a group (§5.4 "Zero extents");
            // a safe-area root is then laid out in the safe area's frame like any other.
            let frame = if safe_root {
                self.safe_area_frame(k)
            } else {
                None
            };
            self.layout_not_fitted(k, frame.as_ref());
            return;
        }
        // *P* in the parent's layout coordinates (§5.4 "Reference box").
        let (wu, hu) = self.screen_size;
        let p_box = if fit_root {
            IBox::from_xywh(0, 0, wu, hu)
        } else if safe_root {
            let a = self.safe_area_box();
            IBox::from_xywh(round_i32(a.l), 0, round_i32(a.w), hu)
        } else {
            match parent.and_then(|p| self.el(p)) {
                Some(pe) => IBox::from_xywh(0, 0, pe.bx.w(), pe.bx.h()),
                None => IBox::from_xywh(0, 0, wu, hu),
            }
        };
        let (p_w, p_h) = (p_box.w(), p_box.h());
        if p_w < 1 || p_h < 1 {
            // A zero extent of P leaves the whole group not fitted (the members share P). A
            // safe-area root's *P* is the safe area, never empty.
            self.layout_not_fitted(k, None);
            return;
        }
        let keys: Vec<ElementKey> = if safe_root {
            vec![k]
        } else {
            self.cached_group(k)
        };
        let members: Vec<(ElementKey, IBox)> = keys
            .iter()
            .filter_map(|m| self.el(*m).map(|e| (*m, e.design)))
            .collect();
        let rects: Vec<FitMember> = members
            .iter()
            .map(|(_, d)| FitMember {
                left: f64::from(d.l),
                top: f64::from(d.t),
                w: f64::from(d.w()),
                h: f64::from(d.h()),
            })
            .collect();
        let u = scaling::group_box(&rects);
        if u.2 <= 0.0 || u.3 <= 0.0 {
            self.layout_not_fitted(k, None);
            return;
        }
        let content = self.group_content(&members, u);
        let (parent_tl, m) = if fit_root {
            ((0.0, 0.0), (1.0, 1.0))
        } else {
            self.parent_frame(parent)
        };
        let a_in = scaling::safe_area_in_parent(self.safe_area_box(), parent_tl, m);
        let (px0, py0) = (f64::from(p_box.l), f64::from(p_box.t));
        let p_d = DBox::new(px0, py0, f64::from(p_w), f64::from(p_h));
        let g = {
            let i = p_d.intersect(&a_in);
            if i.is_empty() { p_d } else { i }
        };
        // `fit_type_a` works with *P* at its own origin.
        let g_local = DBox::new(g.l - px0, g.t - py0, g.w, g.h);
        let grp = scaling::fit_type_a(
            f64::from(p_w),
            f64::from(p_h),
            u,
            content,
            g_local,
            scaling::RHO,
        );
        for (x, d) in members {
            let mut mf = grp.member(u.0, u.1, f64::from(d.l), f64::from(d.t));
            mf.x += px0;
            mf.y += py0;
            // A Type A container is never placed in a layout frame (its *P* is given in the
            // parent's coordinates, §5.3 backdrop, §5.4).
            self.frame_offsets.remove(&x);
            self.apply_fit(x, mf, d.w(), d.h());
        }
    }

    // ----- fit groups (`ui-scaling.md` §5.4 "Fit groups") --------------------------------
    //
    // The grouping relation is an equivalence with a key that each container computes on its
    // own: with *Z* the nearest element at or above a container's parent that is a fit
    // container, carries a node scale or is the root panel (the **anchor**), two non-root
    // Type A containers are grouped exactly when they have the same anchor, their parents the
    // same design size and the same current box relative to *Z*. (Their lowest common ancestor
    // *Q* lies at or below *Z* on both paths, every element strictly between *Q* and either
    // container is then neither a fit container nor scaled, and equal boxes relative to *Q*
    // are equal boxes relative to *Z*; conversely two containers whose paths admit such a *Q*
    // meet the first non-free element above it at the same *Z*.) Fit roots are keyed by their
    // layout. A group is therefore one bucket of a hash partition of the containers of one
    // scope (a root panel, or the screen for fit roots), found in one pass, never by pairwise
    // search; the layout path caches the partition and checks it on use.

    /// The members of `k`'s Type A fit group (`ui-scaling.md` §5.4 "Fit groups"), `k` first,
    /// each with its design rectangle in *P*'s design coordinates; the rest in drawing order.
    /// Empty when `k` is not a group candidate (not a Type A container attached to the screen,
    /// or one with a zero design extent, which is not fitted, §5.4 "Zero extents"). Computed
    /// fresh from the current tree (one pass over `k`'s scope); members are taken shown or not.
    /// Bounded by [`MAX_GROUP_MEMBERS`] and [`MAX_GROUP_SCAN`] (Tessera caps, hostile trees).
    pub(crate) fn type_a_group(&self, k: ElementKey) -> Vec<(ElementKey, IBox)> {
        let Some(key) = self.group_key(k) else {
            return Vec::new();
        };
        let members = match self.group_scope(k, key) {
            Some(scope) => {
                let part = self.scan_partition(scope);
                part.group_of(k).unwrap_or_else(|| vec![k])
            }
            None => vec![k],
        };
        let mut out: Vec<(ElementKey, IBox)> = Vec::with_capacity(members.len());
        if let Some(e) = self.el(k) {
            out.push((k, e.design));
        }
        out.extend(
            members
                .iter()
                .filter(|m| **m != k)
                .filter_map(|m| self.el(*m).map(|e| (*m, e.design))),
        );
        out
    }

    /// Whether `k` is a non-root Type A container that can take part in a fit group: object
    /// mode Stretch, both horizontal edges 6, a design size of at least 1 × 1, attached.
    pub(crate) fn is_type_a_member(&self, k: ElementKey) -> bool {
        let Some(e) = self.el(k) else { return false };
        e.parent.is_some()
            && e.design.w() >= 1
            && e.design.h() >= 1
            && !self.is_fit_root(k)
            && self.fit_kind(k) == Some(FitKind::A)
    }

    /// Whether `k` is a fit root that can take part in a fit group (a design size of at least
    /// 1 × 1; a zero extent is not fitted, §5.4 "Zero extents").
    fn is_fit_root_member(&self, k: ElementKey) -> bool {
        self.is_fit_root(k)
            && self
                .el(k)
                .is_some_and(|e| e.design.w() >= 1 && e.design.h() >= 1)
    }

    /// The grouping key of a fit-group candidate (see the section comment above), or `None`
    /// when `k` is no candidate. A Type A root that is a safe-area panel, or one outside any
    /// root panel, is keyed by itself (a group of one).
    fn group_key(&self, k: ElementKey) -> Option<GroupKey> {
        if self.is_fit_root_member(k) {
            return self.el(k).map(|e| GroupKey::FitRoot(e.layout_did.0));
        }
        if !self.is_type_a_member(k) {
            return None;
        }
        let Some(px) = self.parent(k) else {
            return Some(GroupKey::Alone(k));
        };
        if self.el(k).is_some_and(|e| e.root_panel) || px == self.screen {
            return Some(GroupKey::Alone(k));
        }
        // The anchor: the nearest element at or above the parent that is a fit container,
        // carries a node scale or is the root panel.
        let mut z = px;
        let mut n = 0usize;
        loop {
            if z == self.screen {
                // Not inside a root panel: never grouped.
                return Some(GroupKey::Alone(k));
            }
            let e = self.el(z)?;
            if e.root_panel || self.fit_kind(z).is_some() || self.carries_node_scale(z) {
                break;
            }
            let Some(p) = e.parent else {
                return Some(GroupKey::Alone(k));
            };
            z = p;
            n += 1;
            if n > MAX_TREE_DEPTH * 4 {
                return Some(GroupKey::Alone(k));
            }
        }
        let rel = self.box_relative_to(px, z)?;
        let d = self.design_box(px)?;
        Some(GroupKey::Inner {
            anchor: z,
            parent_box: rel,
            parent_design: (d.w(), d.h()),
        })
    }

    /// The scope a candidate's group is searched in: the screen root for fit roots, the root
    /// panel for other Type A containers; `None` for a container keyed by itself.
    fn group_scope(&self, k: ElementKey, key: GroupKey) -> Option<ElementKey> {
        match key {
            GroupKey::FitRoot(_) => Some(self.screen),
            GroupKey::Alone(_) => None,
            GroupKey::Inner { .. } => self.root_panel_of(k),
        }
    }

    /// The fit-group partition of one scope (`ui-scaling.md` §5.4), in one pass: the fit roots
    /// among the screen root's children (scope = the screen root) or the Type A containers in a
    /// root panel's subtree, pre-order (drawing order), bucketed by [`GroupKey`]; the member
    /// cap is applied by [`Partition::groups`]. At most [`MAX_GROUP_SCAN`] elements are visited;
    /// candidates beyond are not in the partition.
    fn scan_partition(&self, scope: ElementKey) -> Partition {
        let mut part = Partition::default();
        let mut index: std::collections::HashMap<GroupKey, usize> =
            std::collections::HashMap::new();
        let mut add = |part: &mut Partition, c: ElementKey, key: GroupKey| {
            part.reached.push(c);
            match index.get(&key).and_then(|&i| part.buckets.get_mut(i)) {
                Some(b) => b.1.push(c),
                None => {
                    index.insert(key, part.buckets.len());
                    part.buckets.push((key, vec![c]));
                }
            }
        };
        if scope == self.screen {
            for (i, c) in self.children(self.screen).into_iter().enumerate() {
                if i >= MAX_GROUP_SCAN {
                    part.truncated = true;
                    break;
                }
                if let Some(key) = self.group_key(c)
                    && matches!(key, GroupKey::FitRoot(_))
                {
                    add(&mut part, c, key);
                }
            }
            return part;
        }
        let mut stack: Vec<ElementKey> = self.children(scope).into_iter().rev().collect();
        let mut visited = 0usize;
        while let Some(c) = stack.pop() {
            visited += 1;
            if visited > MAX_GROUP_SCAN {
                part.truncated = true;
                break;
            }
            if let Some(key) = self.group_key(c)
                && matches!(key, GroupKey::Inner { .. })
            {
                add(&mut part, c, key);
            }
            if let Some(e) = self.el(c) {
                stack.extend(e.children.iter().rev().copied());
            }
        }
        part
    }

    /// The members of `k`'s fit group for the layout path (`k` included, drawing order), from
    /// the cached partition of its scope when it still holds: `k` and every cached member are
    /// still candidates with the cached key. Otherwise the scope is partitioned again in one
    /// pass and cached. So laying out every member of every group costs one pass per scope plus
    /// a check of at most [`MAX_GROUP_MEMBERS`] keys per member, never a search per member.
    fn cached_group(&mut self, k: ElementKey) -> Vec<ElementKey> {
        let Some(key) = self.group_key(k) else {
            return vec![k];
        };
        let Some(scope) = self.group_scope(k, key) else {
            return vec![k];
        };
        if let Some(c) = self.fit_groups.of.get(&k)
            && c.key == key
            && c.members.contains(&k)
            && c.members.iter().all(|m| self.group_key(*m) == Some(key))
        {
            return c.members.to_vec();
        }
        if !self.fit_groups.of.contains_key(&k)
            && self
                .fit_groups
                .scopes
                .get(&scope)
                .is_some_and(|s| s.truncated)
        {
            // Beyond the scan cap of a hostile scope: a group of one, kept until the scope is
            // partitioned again.
            let members: std::sync::Arc<[ElementKey]> = std::sync::Arc::from(vec![k]);
            self.fit_groups.of.insert(
                k,
                CachedGroup {
                    key,
                    members: members.clone(),
                },
            );
            return members.to_vec();
        }
        let part = self.scan_partition(scope);
        if part.truncated {
            tracing::warn!(
                cap = MAX_GROUP_SCAN,
                "ui-scaling §5.4: a fit-group scan stopped at its cap; later containers are fitted \
                 as groups of one"
            );
        }
        if part.capped() {
            tracing::warn!(
                cap = MAX_GROUP_MEMBERS,
                "ui-scaling §5.4: a fit group exceeds its member cap; the members past it are \
                 fitted as groups of one"
            );
        }
        if let Some(old) = self.fit_groups.scopes.remove(&scope) {
            for m in old.reached {
                self.fit_groups.of.remove(&m);
            }
        }
        for (gk, members) in &part.groups() {
            let shared: std::sync::Arc<[ElementKey]> = std::sync::Arc::from(members.clone());
            for m in members.iter() {
                self.fit_groups.of.insert(
                    *m,
                    CachedGroup {
                        key: *gk,
                        members: shared.clone(),
                    },
                );
            }
        }
        let found = self
            .fit_groups
            .of
            .get(&k)
            .filter(|c| c.key == key)
            .map(|c| c.members.to_vec());
        let mut reached = part.reached;
        let members = match found {
            Some(m) => m,
            None => {
                // `k` lies beyond the scan cap: a group of one.
                reached.push(k);
                self.fit_groups.of.insert(
                    k,
                    CachedGroup {
                        key,
                        members: std::sync::Arc::from(vec![k]),
                    },
                );
                vec![k]
            }
        };
        self.fit_groups.scopes.insert(
            scope,
            ScopeScan {
                reached,
                truncated: part.truncated,
            },
        );
        members
    }

    /// Whether an element carries a node scale other than 1 (`ui-scaling.md` §3.4).
    fn carries_node_scale(&self, k: ElementKey) -> bool {
        self.el(k)
            .and_then(|e| e.scale)
            .is_some_and(|s| s != [1.0, 1.0, 1.0])
    }

    /// An element's current layout box made relative to its ancestor `q` (`(0, 0, q's width,
    /// q's height)` for `q` itself), as `(l, t, w, h)`.
    fn box_relative_to(&self, k: ElementKey, q: ElementKey) -> Option<(i32, i32, i32, i32)> {
        let qb = self.el(q)?.bx;
        if k == q {
            return Some((0, 0, qb.w(), qb.h()));
        }
        let mut b = self.el(k)?.bx;
        let mut c = self.parent(k)?;
        let mut n = 0;
        while c != q {
            let cb = self.el(c)?.bx;
            b = b.offset(cb.l, cb.t);
            c = self.parent(c)?;
            n += 1;
            if n > MAX_TREE_DEPTH * 4 {
                return None;
            }
        }
        Some((b.l, b.t, b.w(), b.h()))
    }

    /// The content box *C* of a Type A fit group (`ui-scaling.md` §5.4 "Group boxes"): the
    /// smallest box holding, for every member, its direct children with a width and height of
    /// at least 1, each cut to the member's design rectangle and taken into *P*'s design
    /// coordinates; relative to the group box *U*'s top-left. `None` when empty.
    fn group_content(
        &self,
        members: &[(ElementKey, IBox)],
        u: (f64, f64, f64, f64),
    ) -> Option<ContentBox> {
        let mut l = f64::INFINITY;
        let mut t = f64::INFINITY;
        let mut r = f64::NEG_INFINITY;
        let mut b = f64::NEG_INFINITY;
        for (m, md) in members {
            let (dw, dh) = (md.w(), md.h());
            let Some(me) = self.el(*m) else { continue };
            for c in &me.children {
                let Some(e) = self.el(*c) else { continue };
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
                let (ox, oy) = (f64::from(md.l), f64::from(md.t));
                l = l.min(ox + f64::from(cl));
                t = t.min(oy + f64::from(ct));
                r = r.max(ox + f64::from(cr));
                b = b.max(oy + f64::from(cb));
            }
        }
        if !l.is_finite() || !t.is_finite() || !r.is_finite() || !b.is_finite() {
            return None;
        }
        Some(ContentBox {
            x: l - u.0,
            y: t - u.1,
            w: r - l,
            h: b - t,
        })
    }

    /// Fits a Type B or C container (`ui-scaling.md` §5.4): the box UL §8.4 steps 2–5 give in
    /// the box it is laid out in (its parent's current box, the §5.3 safe box of a full-screen
    /// root, or for a root panel the safe area's frame, §5.2) drives one axis. A zero driving
    /// extent leaves it not fitted.
    fn layout_type_bc(&mut self, k: ElementKey, kind: FitKind, frame: Option<Frame>) {
        let Some(e) = self.el(k) else { return };
        let edges = e.edges;
        let design = e.design;
        let parent = e.parent;
        let (dw, dh) = (design.w(), design.h());
        if dw < 1 || dh < 1 {
            self.layout_not_fitted(k, frame.as_ref());
            return;
        }
        let (c, p_w, p_h) = match &frame {
            Some(fr) => (self.frame_box(k, fr), fr.new.w(), fr.new.h()),
            None => {
                let (p_w, p_h) = match parent.and_then(|p| self.el(p)) {
                    Some(pe) if parent != Some(self.screen) => (pe.bx.w(), pe.bx.h()),
                    _ => self.screen_size,
                };
                (self.fit_computed_box(k), p_w, p_h)
            }
        };
        let offset = frame.as_ref().map_or((0, 0), |f| f.offset);
        let computed = ComputedBox {
            left: f64::from(c.l),
            top: f64::from(c.t),
            right: f64::from(c.r),
            bottom: f64::from(c.b),
        };
        let fit = if kind == FitKind::B {
            // The height drives; left edge 6 → anchored at the computed right, right edge 6 →
            // at the computed left.
            if c.h() < 1 || p_w < 1 {
                self.layout_not_fitted(k, frame.as_ref());
                return;
            }
            scaling::fit_type_b(
                f64::from(c.h()),
                f64::from(p_w),
                f64::from(dw),
                f64::from(dh),
                computed,
                edges[0] == 6,
            )
        } else {
            // The width drives.
            if c.w() < 1 || p_h < 1 {
                self.layout_not_fitted(k, frame.as_ref());
                return;
            }
            let anchor = match (edges[1] == 6, edges[3] == 6) {
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
        };
        if !(fit.f.is_finite() && fit.f > 0.0) {
            self.layout_not_fitted(k, frame.as_ref());
            return;
        }
        // `frame_box` already added the frame offset to the computed edges; record it so the
        // next layout takes it out again (§5.2 "Layout frames").
        self.record_frame_offset(k, offset);
        self.apply_fit(k, fit, dw, dh);
    }

    /// The box UL §8.4 steps 2–5 give for an element in its parent's current box, without
    /// setting it (`ui-scaling.md` §5.4 Types B and C read this to drive their one axis). It is
    /// the plain run-time layout of [`Ui::layout_default`] computed but not applied.
    fn fit_computed_box(&self, k: ElementKey) -> IBox {
        let (old, new) = self.reference_boxes(k);
        let prev = self.frame_offsets.get(&k).copied().unwrap_or((0, 0));
        let Some(e) = self.el(k) else {
            return IBox::default();
        };
        let inp = RuntimeInput {
            design: e.design,
            current: e.bx.offset(-prev.0, -prev.1),
            edges: e.edges,
            old,
            new,
            finished: e.created,
            keep_inside: e.flags & 0x02 != 0,
        };
        runtime(&inp)
    }

    /// The fit-to-screen factor *q* of a root panel (`ui-scaling.md` §5.6): when its displayed
    /// box, with its own node scale (a per-window factor, a plugin `SetScale` factor, a stretch
    /// vector, a fit factor of a root that is a fit container), is wider than *Wu* or taller
    /// than *Hu*, *q* = min(1, *Wu* ÷ width, *Hu* ÷ height) multiplies that scale, so the panel
    /// fits; *q* is 1 again as soon as it fits. The root's layout size (a plugin's `GetSize()`)
    /// does not change. The scale *q* multiplied is kept, so recomputing never compounds; a
    /// scale set by other code since replaces it. Runs only under a UI screen, for native root
    /// panels (modal dialogs included) and plugin windows; fit roots are exempt (§5.6). When the
    /// scale changes, the fit containers inside the root are fitted again.
    pub(crate) fn refresh_fit_to_screen(&mut self, k: ElementKey) {
        if self.ui_screen.is_none() || !self.is_screen_root(k) {
            return;
        }
        if self.is_fit_root(k) {
            self.fit_q.remove(&k);
            return;
        }
        let Some(e) = self.el(k) else { return };
        let cur = e.scale;
        let b = e.bx;
        let base = match self.fit_q.get(&k) {
            Some(r) if r.written == cur => r.base,
            _ => cur,
        };
        let (bx, by) = base.map_or((1.0, 1.0), |s| (f64::from(s[0]), f64::from(s[1])));
        let (wu, hu) = self.screen_size;
        let q = scaling::fit_to_screen(
            f64::from(b.w().max(0)) * bx,
            f64::from(b.h().max(0)) * by,
            wu,
            hu,
        );
        let new = if q < 1.0 {
            let qf = q as f32;
            let s = base.unwrap_or([1.0, 1.0, 1.0]);
            Some([s[0] * qf, s[1] * qf, s[2]])
        } else {
            base
        };
        if q < 1.0 {
            self.fit_q.insert(k, FitQ { base, written: new });
        } else {
            self.fit_q.remove(&k);
        }
        if new != cur {
            if let Some(e) = self.el_mut(k) {
                e.scale = new;
            }
            self.painter.note_structure();
            self.mark_dirty(k);
            self.input.hover_dirty = true;
            self.refit_descendants(k);
        }
    }

    /// The fit-to-screen factor *q* a root panel carries now (`ui-scaling.md` §5.6): 1 when it
    /// fits the UI screen (or at the identity).
    #[must_use]
    pub fn fit_to_screen_factor(&self, k: ElementKey) -> f32 {
        match (self.fit_q.get(&k), self.el(k).and_then(|e| e.scale)) {
            (Some(r), Some(s)) if r.written == Some(s) => {
                let b = r.base.map_or(1.0, |b| b[0]);
                if b != 0.0 && b.is_finite() {
                    s[0] / b
                } else {
                    1.0
                }
            }
            _ => 1.0,
        }
    }

    /// The uniform fit factor *f* of a fit container (`ui-scaling.md` §5.4), or 1 for any other
    /// element (and at the identity). This is the container's own factor, without *m*(parent)
    /// or a fit-to-screen factor, although its node scale carries their product (§3.4).
    #[must_use]
    pub fn fit_factor(&self, k: ElementKey) -> f32 {
        if self.ui_screen.is_none() || self.fit_kind(k).is_none() {
            return 1.0;
        }
        self.fit_f.get(&k).copied().unwrap_or(1.0)
    }

    /// Whether a root panel's displayed box lies entirely inside the UI screen *F* now
    /// (`ui-scaling.md` §5.8 item 5).
    fn root_inside_screen(&self, k: ElementKey) -> bool {
        let d = self.displayed_box(k);
        let (wu, hu) = self.screen_size;
        d.l >= 0 && d.t >= 0 && d.r < wu && d.b < hu
    }

    /// Lays out again what depends on *F* or *A* besides the parent's size (`ui-scaling.md`
    /// §4.5 step 2): the root panels (modal dialogs included) and the §5.3 children of
    /// full-screen roots (the safe box follows *A*) when the screen root kept its size
    /// (`resize_screen` already did them otherwise), and every fit container, parents first,
    /// because a Type A guard box is *A* taken into the parent's coordinates (§5.4 "Nesting and
    /// fitting order"), which moves with *F* and *A* even where the parent's size does not. On
    /// the first call the children of Stretch roots are laid out too (the UL §8.4 step 1 skip no
    /// longer applies, §5.2).
    fn relayout_for_screen_change(&mut self, first: bool, f_changed: bool) {
        for r in self.layout_roots() {
            if self.el(r).is_none_or(|e| e.plugin.is_some()) {
                continue;
            }
            if !f_changed {
                self.layout_element(r);
            }
            // The §5.3 children of a full-screen root follow A; the children of a Stretch root
            // skipped by UL §8.4 step 1 at the identity are laid out once (§5.2).
            if (!f_changed && self.is_full_screen_root(r))
                || (first && self.object_mode(r) == Some(object_mode::STRETCH))
            {
                self.relayout_children(r);
            }
        }
        let mut stack: Vec<ElementKey> = self.children(self.screen).into_iter().rev().collect();
        let mut n = 0usize;
        while let Some(c) = stack.pop() {
            n += 1;
            if n > crate::ui::MAX_ELEMENTS {
                break;
            }
            if !self.is_screen_root(c) && self.fit_kind(c).is_some() {
                self.layout_element(c);
            }
            if let Some(e) = self.el(c) {
                stack.extend(e.children.iter().rev().copied());
            }
        }
        self.input.hover_dirty = true;
    }

    /// Runs the §5.8 item-3 steps after a change of *Se*, *F*, *A* (`ui-scaling.md` §4.5 step 2,
    /// §5.8): every native root panel (modal dialogs included, fit roots excepted) gets its
    /// fit-to-screen factor recomputed (§5.6), its saved placement applied again when *F* or *A*
    /// changed (`reapply`, §5.8 "Applying"; not after a change of *Se* alone), and is kept on
    /// screen; a plugin window is kept on screen only when it lay entirely inside the old *F*
    /// (`inside`), so one placed partly off screen with `SetPosition` stays where the plugin put
    /// it.
    fn after_screen_change(&mut self, inside: &[ElementKey], reapply: bool) {
        for r in self.layout_roots() {
            let Some(e) = self.el(r) else { continue };
            let plugin = e.plugin.is_some();
            self.refresh_fit_to_screen(r);
            if plugin {
                if inside.contains(&r) {
                    self.keep_root_on_screen(r);
                }
                continue;
            }
            if self.is_fit_root(r) {
                continue;
            }
            if reapply {
                self.reapply_saved(r);
            }
            self.keep_root_on_screen(r);
        }
        self.input.hover_dirty = true;
    }

    /// Runs §5.6 and keep-on-screen after a change of a root panel's own node scale
    /// (`ui-scaling.md` §5.8 item 3: a per-window scale); `was_inside` tells whether it lay
    /// inside *F* before (plugin windows are kept on screen only then, item 5).
    fn after_scale_change(&mut self, k: ElementKey, was_inside: bool) {
        if self.ui_screen.is_none() || !self.is_screen_root(k) {
            return;
        }
        self.refresh_fit_to_screen(k);
        let plugin = self.el(k).is_some_and(|e| e.plugin.is_some());
        if (!plugin || was_inside) && !self.is_fit_root(k) {
            self.keep_root_on_screen(k);
        }
    }

    // ----- letterbox bars (`ui-scaling.md` §5.4 step 6) ----------------------------------

    /// The letterbox bars of every fit group that draws them (`ui-scaling.md` §5.4 step 6), in
    /// **physical pixels**, clipped to the window: for a fit-root group (*P* = *F*) and for a
    /// group whose *P* is a full-screen root's full box (the root, or a descendant whose box
    /// equals the root's), the part of *P* that no shown member's physical box covers. Each
    /// group's bars are reported once, to be drawn as opaque black fills directly below the
    /// picture of its lowest shown member ([`scaling::LetterboxBars::below`]); a group with no
    /// shown member draws none. A group larger than the member cap is still one group here
    /// (one set of bars, every shown member cut out). Empty at the identity (no UI screen).
    /// One pass per scope.
    ///
    /// The paint pass puts these bars into the composition list as opaque black
    /// [`EntryTexture::Fill`](crate::paint::EntryTexture::Fill) entries, each group's bars
    /// directly below the entry of its lowest shown member, so the UI host draws them with the
    /// rest of the list; a CPU compositor or test may also read them here. Pointer hits in the
    /// bars of a fit-root group go to the group's lowest shown fit root ([`Ui::hit_test`]).
    #[must_use]
    pub fn letterbox_bars(&self) -> Vec<scaling::LetterboxBars> {
        let mut out = Vec::new();
        let Some(f) = self.ui_screen else {
            return out;
        };
        let (wp, hp) = f.physical();
        let window = crate::paint::Rect::new(0, 0, wp, hp);
        for (_, members) in self.scan_partition(self.screen).buckets {
            self.push_bars(&members, window, window, &mut out);
        }
        for r in self.layout_roots() {
            if !self.is_full_screen_root(r) {
                continue;
            }
            let root_disp = self.displayed_box(r);
            for (_, members) in self.scan_partition(r).buckets {
                let Some(p) = members.first().and_then(|m| self.parent(*m)) else {
                    continue;
                };
                // P is the members' shared parent box; bars only when it is the root's box.
                if p == r || self.displayed_box(p) == root_disp {
                    let p_phys = self.physical_box(p);
                    self.push_bars(&members, p_phys, window, &mut out);
                }
            }
        }
        out
    }

    /// Adds one group's bars: `p_phys` minus the physical boxes of the shown members, clipped
    /// to the window, below the lowest shown member (`members` are in drawing order).
    fn push_bars(
        &self,
        members: &[ElementKey],
        p_phys: crate::paint::Rect,
        window: crate::paint::Rect,
        out: &mut Vec<scaling::LetterboxBars>,
    ) {
        let shown: Vec<ElementKey> = members
            .iter()
            .copied()
            .filter(|m| self.is_displayed(*m))
            .collect();
        let Some(lowest) = shown.first().copied() else {
            return;
        };
        let holes: Vec<crate::paint::Rect> = shown.iter().map(|m| self.physical_box(*m)).collect();
        let rects = scaling::letterbox_rects(p_phys.intersect(&window), &holes);
        if !rects.is_empty() {
            out.push(scaling::LetterboxBars {
                below: lowest,
                rects,
            });
        }
    }

    /// The fit roots whose group's letterbox bars contain the screen position (`x`, `y`) (real
    /// UI units), each being its group's **lowest shown** member (`ui-scaling.md` §5.4 step 6,
    /// invariant I3): the bars of a fit-root group count as part of that root for hit testing,
    /// so the pointer hits it and never reaches anything behind a full-screen screen. Worked
    /// out once per hit test, in one pass over the screen's fit roots.
    pub(crate) fn fit_root_bar_owners(&self, x: f64, y: f64) -> Vec<ElementKey> {
        let mut out = Vec::new();
        if self.ui_screen.is_none() {
            return out;
        }
        let (wu, hu) = self.screen_size;
        if !(x >= 0.0 && y >= 0.0 && x < f64::from(wu) && y < f64::from(hu)) {
            return out;
        }
        for (_, members) in self.scan_partition(self.screen).buckets {
            let shown: Vec<ElementKey> = members
                .into_iter()
                .filter(|m| self.is_displayed(*m))
                .collect();
            let Some(lowest) = shown.first().copied() else {
                continue;
            };
            if self
                .el(lowest)
                .is_none_or(|e| !e.visible || e.fade.is_some())
            {
                continue;
            }
            // The real displayed boxes (§3.4), as the drawn member pictures are (§3.6).
            if !shown.iter().any(|m| self.real_edges(*m).0.contains(x, y)) {
                out.push(lowest);
            }
        }
        out
    }
}

/// The fit-container types of `ui-scaling.md` §5.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FitKind {
    /// Both horizontal edges 6 (or a fit root): fitted in groups.
    A,
    /// Exactly one horizontal edge 6: the height drives.
    B,
    /// No horizontal edge 6, a vertical one: the width drives.
    C,
}

/// A layout frame (`ui-scaling.md` §5.2 "Layout frames"): the reference boxes *O* and *N* of
/// UL §8.4 in the frame's own coordinates and the frame's offset inside the parent.
#[derive(Debug, Clone, Copy)]
struct Frame {
    old: IBox,
    new: IBox,
    offset: (i32, i32),
}

/// The unrounded scaling-origin point of a root panel with a per-window factor
/// (`ui-scaling.md` §7.4, [`Ui::set_window_scale`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct WindowAnchor {
    /// The scaling origin's position in the parent's layout coordinates (UI units for a root
    /// panel), unrounded.
    pub(crate) x: f64,
    pub(crate) y: f64,
    /// The layout top-left the panel had when this was recorded; another position means the
    /// panel was moved since, and the anchor is taken again from where it is.
    pub(crate) at: (i32, i32),
}

/// The fit-to-screen record of a root panel (`ui-scaling.md` §5.6): the node scale *q*
/// multiplies (`None` = 1) and the scale written with *q* applied, so a later scale set by
/// other code is detected and taken as the new base.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FitQ {
    pub(crate) base: Option<[f32; 3]>,
    pub(crate) written: Option<[f32; 3]>,
}

/// The key of the §5.4 grouping relation (see the "fit groups" section of [`Ui`]'s fit rules).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum GroupKey {
    /// A fit root, grouped with the other fit roots of the same layout.
    FitRoot(u32),
    /// A non-root Type A container: its anchor, its parent's box relative to the anchor and its
    /// parent's design size.
    Inner {
        anchor: ElementKey,
        parent_box: (i32, i32, i32, i32),
        parent_design: (i32, i32),
    },
    /// A container that is never grouped (a group of one).
    Alone(ElementKey),
}

/// One scope's partition of fit-group candidates by [`GroupKey`] (drawing order inside each
/// bucket). A bucket is a whole group of the §5.4 relation; [`Partition::groups`] applies the
/// member cap to it.
#[derive(Debug, Default)]
struct Partition {
    buckets: Vec<(GroupKey, Vec<ElementKey>)>,
    /// Every candidate reached by the scan.
    reached: Vec<ElementKey>,
    /// The scan stopped at [`MAX_GROUP_SCAN`].
    truncated: bool,
}

impl Partition {
    /// The fit groups: each bucket's first [`MAX_GROUP_MEMBERS`] members in drawing order, and
    /// every member past the cap as a group of one (the same cut for every member, so all the
    /// members of one group share one factor whichever of them is laid out).
    fn groups(&self) -> Vec<(GroupKey, Vec<ElementKey>)> {
        let mut out = Vec::with_capacity(self.buckets.len());
        for (k, m) in &self.buckets {
            let (head, tail) = m.split_at(m.len().min(MAX_GROUP_MEMBERS));
            out.push((*k, head.to_vec()));
            out.extend(tail.iter().map(|c| (*k, vec![*c])));
        }
        out
    }

    /// Whether some bucket exceeds [`MAX_GROUP_MEMBERS`].
    fn capped(&self) -> bool {
        self.buckets
            .iter()
            .any(|(_, m)| m.len() > MAX_GROUP_MEMBERS)
    }

    /// The members of `k`'s fit group (capped), when the scan reached `k`.
    fn group_of(&self, k: ElementKey) -> Option<Vec<ElementKey>> {
        let (_, m) = self.buckets.iter().find(|(_, m)| m.contains(&k))?;
        match m.iter().position(|c| *c == k) {
            Some(i) if i >= MAX_GROUP_MEMBERS => Some(vec![k]),
            _ => Some(m.iter().take(MAX_GROUP_MEMBERS).copied().collect()),
        }
    }
}

/// A cached fit group of one member (shared by all members of the group).
#[derive(Debug, Clone)]
struct CachedGroup {
    key: GroupKey,
    members: std::sync::Arc<[ElementKey]>,
}

/// The last scan of one scope: the candidates it reached and whether it stopped at its cap.
#[derive(Debug, Clone, Default)]
struct ScopeScan {
    reached: Vec<ElementKey>,
    truncated: bool,
}

/// The layout path's cache of fit groups (`ui-scaling.md` §5.4), checked on every use
/// ([`Ui::cached_group`]), so it is never trusted after the tree changed under it.
#[derive(Debug, Default)]
pub(crate) struct GroupCache {
    of: std::collections::HashMap<ElementKey, CachedGroup>,
    scopes: std::collections::HashMap<ElementKey, ScopeScan>,
}

impl GroupCache {
    /// Drops what the cache holds about a destroyed element.
    pub(crate) fn forget(&mut self, k: ElementKey) {
        self.of.remove(&k);
        self.scopes.remove(&k);
    }
}

/// Most members of one Type A fit group (Tessera cap, hostile trees; the install's largest
/// group has 2, `ui-scaling.md` SC13). A larger bucket keeps its first members in drawing order
/// as the group; the rest are groups of one, the same for every member.
pub(crate) const MAX_GROUP_MEMBERS: usize = 64;

/// Most elements one fit-group scan visits (a root panel's subtree, or the screen root's
/// children for fit roots); Tessera cap, hostile trees.
pub(crate) const MAX_GROUP_SCAN: usize = 4096;

/// The safe area as the UI host passed it, clamped into the UI screen `wu` × `hu`; a
/// non-finite or empty one is the whole screen (`ui-scaling.md` §5.1: *A* lies inside *F*).
fn sanitize_safe(a: DBox, wu: i32, hu: i32) -> DBox {
    let f = DBox::new(0.0, 0.0, f64::from(wu), f64::from(hu));
    if !(a.l.is_finite() && a.t.is_finite() && a.w.is_finite() && a.h.is_finite()) {
        return f;
    }
    let i = a.intersect(&f);
    if i.is_empty() { f } else { i }
}

/// Rounds a real UI coordinate to the nearest integer layout unit, halves up (`ui-scaling.md`
/// §3.5 "Moves and resizes"). A non-finite value becomes 0; the result is clamped into `i32`.
fn round_i32(v: f64) -> i32 {
    crate::geom::round_half_up(v)
}
