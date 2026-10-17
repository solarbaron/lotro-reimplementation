// SPDX-License-Identifier: GPL-3.0-or-later
//! UI scale (`ui-behaviour.md` §9.2) and screen size changes (§9.4, `ui-layout.md` §8.5).

use tessera_formats::property::PropertyValue;

use crate::events::UiEvent;
use crate::geom::{IBox, add, sub, trunc_f32};
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
}
