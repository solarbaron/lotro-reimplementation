// SPDX-License-Identifier: GPL-3.0-or-later
//! Showing and hiding (`ui-behaviour.md` §11.2) and auto-fade (§11.1).

use tessera_formats::property::PropertyValue;

use crate::events::MsgArg;
use crate::tree::{AutoFade, ElementKey};
use crate::ui::{Ui, object_mode};
use crate::vocab::tessera_msg;

impl Ui {
    /// Whether an element wants a surface (decision 0015 Decision 1.1 condition 1).
    pub(crate) fn wants_surface(&self, k: ElementKey) -> bool {
        let Some(e) = self.el(k) else { return false };
        match self.object_mode(k) {
            Some(m) => m != object_mode::NONE,
            None => e.root_panel,
        }
    }

    /// Shows or hides an element as code does (§11.1–§11.2). The logical show takes effect
    /// at once (decision 0015 Decision 1.8).
    pub fn set_shown(&mut self, k: ElementKey, show: bool) {
        let Some(e) = self.el(k) else { return };
        let fading = e.fade;
        let visible = e.visible;
        let af = self.prop(k, self.v.pids.auto_fade);
        let auto = af.is_set() && self.wants_surface(k) && k != self.screen;
        let dur = af.float();
        if show {
            if visible && fading.is_none_or(|f| f.to >= 1.0) {
                return;
            }
            self.set_visible_flag(k, true);
            if auto {
                self.start_fade(k, 1.0, dur);
            } else if let Some(e) = self.el_mut(k) {
                e.fade = None;
            }
            self.on_shown(k);
        } else {
            if !visible {
                return;
            }
            if auto {
                if fading.is_some_and(|f| f.to <= 0.0) {
                    return;
                }
                // Tessera rule (§11.2 implementer note): a surface that has never been drawn
                // (no surface yet, or its first paint still withheld) was never on screen, so
                // it hides at once, as a fade of zero length: the same messages and final
                // opacity 0 (a later show fades in from 0), but no fade-out of a panel the
                // user never saw.
                let dur = if self.is_surface_drawn(k) { dur } else { 0.0 };
                self.start_fade(k, 0.0, dur);
                self.input.hover_dirty = true;
            } else {
                if let Some(e) = self.el_mut(k) {
                    e.fade = None;
                }
                self.hide_now(k);
            }
        }
    }

    fn set_visible_flag(&mut self, k: ElementKey, v: bool) {
        let Some(e) = self.el_mut(k) else { return };
        if e.visible == v {
            return;
        }
        e.visible = v;
        self.mark_dirty(k);
        self.input.hover_dirty = true;
        if self.el(k).is_some_and(|e| e.plugin.is_some()) {
            self.post(k, tessera_msg::VISIBLE_CHANGED, MsgArg::default());
        }
        if self.prop(k, self.v.pids.enable_visibility_broadcast).bool() {
            self.post(
                k,
                self.v.msgs.is_visible_changed,
                MsgArg::value(i64::from(v)),
            );
        }
        // A list box laying out only visible entries is dirty.
        if let Some(p) = self.parent(k)
            && self.el(p).is_some_and(|pe| pe.widget.list.is_some())
            && self.prop(p, self.v.pids.lb_only_visible).bool()
        {
            if let Some(l) = self.el_mut(p).and_then(|e| e.widget.list.as_mut()) {
                l.dirty = true;
            }
            self.list_layout_if_dirty(p);
        }
    }

    fn hide_now(&mut self, k: ElementKey) {
        self.set_visible_flag(k, false);
        // §11.2 hiding: activation list, focus, active panel.
        self.active_hidden(k);
        if self.tooltip.panel == Some(k) {
            self.tooltip.panel = None;
        }
    }

    pub(crate) fn on_shown(&mut self, k: ElementKey) {
        let p = self.v.pids;
        if self.prop(k, p.activate_on_show).bool() {
            self.request_activation(k);
        }
        if self.prop(k, p.focus_on_show).bool() && self.is_displayed(k) {
            self.set_focus(Some(k));
        }
        if let Some((s, scaler_type)) = self.el(k).and_then(|e| e.pending_scale) {
            if let Some(e) = self.el_mut(k) {
                e.pending_scale = None;
            }
            self.apply_panel_scale(k, s, scaler_type);
        }
    }

    fn start_fade(&mut self, k: ElementKey, to: f32, duration: f32) {
        let from = self.prop(k, self.v.pids.object_opacity).float();
        let now = self.now;
        if let Some(e) = self.el_mut(k) {
            e.fade = Some(AutoFade {
                start: now,
                from,
                to,
                duration,
            });
        }
        self.post(k, self.v.msgs.auto_fade_start, MsgArg::default());
        self.want_frame(k);
        self.fade_frame(k);
    }

    /// Per-frame auto-fade (§11.1 step 2–3). Returns whether it keeps running.
    pub(crate) fn fade_frame(&mut self, k: ElementKey) -> bool {
        let Some(f) = self.el(k).and_then(|e| e.fade) else {
            return false;
        };
        let t = if f.duration.abs() <= 0.0002 {
            1.0
        } else {
            (((self.now - f.start) / f64::from(f.duration)) as f32).clamp(0.0, 1.0)
        };
        let o = f.from + (f.to - f.from) * t;
        self.set_fade_opacity(k, o);
        if t >= 1.0 {
            if let Some(e) = self.el_mut(k) {
                e.fade = None;
            }
            self.input.hover_dirty = true;
            self.post(k, self.v.msgs.auto_fade_stop, MsgArg::default());
            if f.to <= 0.0 {
                self.hide_now(k);
            }
            false
        } else {
            true
        }
    }

    /// Writes a fade opacity into `k` and each child with its own surface (§11.1 step 2).
    fn set_fade_opacity(&mut self, k: ElementKey, o: f32) {
        let pid = self.v.pids.object_opacity;
        self.set_property(k, pid, PropertyValue::Float(o));
        let mut stack = self.children(k);
        let mut n = 0;
        while let Some(c) = stack.pop() {
            n += 1;
            if n > 1 << 16 {
                break;
            }
            if self.owns_surface(c) {
                self.set_property(c, pid, PropertyValue::Float(o));
            }
            stack.extend(self.children(c));
        }
    }

    /// A panel opened hidden ([`Ui::open_panel_hidden`]) takes the state a completed fade-out
    /// leaves when it has an auto-fade: opacity 0 on it and its children with surfaces, so
    /// that code's first show fades it in from 0, as after an open and a hide.
    pub(crate) fn opened_hidden(&mut self, k: ElementKey) {
        if self.prop(k, self.v.pids.auto_fade).is_set() && self.wants_surface(k) {
            self.set_fade_opacity(k, 0.0);
        }
    }

    /// Whether the element's own drawing surface has been drawn: it exists and its first
    /// paint is no longer withheld (decision 0015 Decision 1.8). `false` for an element
    /// without a surface, and for one that was opened or shown and not painted since.
    #[must_use]
    pub fn is_surface_drawn(&self, k: ElementKey) -> bool {
        self.surface_of(k)
            .is_some_and(|id| self.painter.scheduler.is_drawn(id))
    }

    /// Whether an element is in an automatic fade (skipped by the hit test, §4.2).
    #[must_use]
    pub fn is_fading(&self, k: ElementKey) -> bool {
        self.el(k).is_some_and(|e| e.fade.is_some())
    }
}
