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
}
