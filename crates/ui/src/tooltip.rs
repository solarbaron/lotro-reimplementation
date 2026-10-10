// SPDX-License-Identifier: GPL-3.0-or-later
//! Tooltips (`ui-behaviour.md` §6) and the hover check that delivers `MouseHover`
//! (`lua-plugin-api.md` §13.12.5).

use tessera_formats::did::Did;
use tessera_formats::property::PropertyValue;
use tessera_formats::string_info::StringSource;

use crate::events::{Answer, MsgArg};
use crate::geom::trunc_f32;
use crate::tree::ElementKey;
use crate::ui::{Ui, object_mode};
use crate::widgets::WidgetClass;

/// Tooltip state.
#[derive(Debug, Clone, Default)]
pub(crate) struct TooltipState {
    /// The hover-handled flag (§6.2 step 2).
    pub handled: bool,
    /// The tooltip panel showing.
    pub panel: Option<ElementKey>,
    /// Further panels shown with it (the comparison tooltips of an examination tooltip,
    /// `item-tooltips.md` §8), hidden and destroyed with it.
    pub extra: Vec<ElementKey>,
}

impl Ui {
    /// The tooltip panel showing, if any.
    #[must_use]
    pub fn tooltip_panel(&self) -> Option<ElementKey> {
        self.tooltip.panel
    }

    /// The panels shown with the tooltip panel (comparison tooltips), in order.
    #[must_use]
    pub fn tooltip_extra_panels(&self) -> &[ElementKey] {
        &self.tooltip.extra
    }

    /// Makes `main` (and `extra`) the showing tooltip, built by game code (an examination
    /// tooltip, `item-tooltips.md` §9): any other tooltip is hidden first, and these panels
    /// are hidden and destroyed by the rules of `ui-behaviour.md` §6.6 from now on. Panels
    /// that do not exist are ignored.
    pub fn set_tooltip_panels(&mut self, main: ElementKey, extra: Vec<ElementKey>) {
        let keep: Vec<ElementKey> = std::iter::once(main).chain(extra.iter().copied()).collect();
        if let Some(p) = self.tooltip.panel
            && keep.contains(&p)
        {
            self.tooltip.panel = None;
        }
        self.tooltip.extra.retain(|x| !keep.contains(x));
        self.hide_tooltip();
        if !self.exists(main) {
            for x in extra {
                self.destroy(x);
            }
            return;
        }
        self.tooltip.panel = Some(main);
        self.tooltip.extra = extra.into_iter().filter(|x| self.exists(*x)).collect();
        for k in std::iter::once(main).chain(self.tooltip.extra.clone()) {
            self.mark_dirty(k);
        }
        self.painter.structure_changed();
    }

    /// Marks an element as asking for an examination tooltip on hover (or clears the mark):
    /// when the hover handler runs for it or a descendant, the UI emits
    /// [`crate::UiEvent::ExamineHover`] instead of showing a text tooltip
    /// (`item-tooltips.md` §9 Tessera rule).
    pub fn set_examine(&mut self, k: ElementKey, on: bool) {
        if let Some(e) = self.el_mut(k) {
            e.examine = on;
        }
        self.recompute_mouse_visible(k);
    }

    /// Whether an element asks for an examination tooltip.
    #[must_use]
    pub fn examines(&self, k: ElementKey) -> bool {
        self.el(k).is_some_and(|e| e.examine)
    }

    /// The nearest element at or above `k` that asks for an examination tooltip.
    fn examine_target(&self, k: ElementKey) -> Option<ElementKey> {
        let mut cur = Some(k);
        let mut n = 0;
        while let Some(c) = cur {
            if self.el(c).is_some_and(|e| e.examine) {
                return Some(c);
            }
            n += 1;
            if n > crate::ui::MAX_TREE_DEPTH * 2 {
                return None;
            }
            cur = self.parent(c);
        }
        None
    }

    /// Whether the element has tooltip text (§6.3 step 1): a run-time string, or a
    /// `UICore_Element_tooltip_entry` that names text.
    pub(crate) fn has_tooltip_text(&self, k: ElementKey) -> bool {
        let Some(e) = self.el(k) else { return false };
        if e.tooltip_text.as_ref().is_some_and(|t| !t.is_empty()) {
            return true;
        }
        match self.property(k, self.v.pids.tooltip_entry) {
            Some(PropertyValue::StringInfo(si)) => match &si.source {
                StringSource::Literal(s) => !s.is_empty(),
                StringSource::Table { token, table } => *token != 0 || table.0 != 0,
            },
            _ => false,
        }
    }

    /// Sets the run-time tooltip text (§6.3 step 1); recomputes mouse-visible (§4.3).
    pub fn set_tooltip_text(&mut self, k: ElementKey, text: Option<String>) {
        if let Some(e) = self.el_mut(k) {
            e.tooltip_text = text;
        }
        self.recompute_mouse_visible(k);
    }

    /// Marks an element for hover messages (§6.2 step 4.2).
    pub fn set_hover_messages(&mut self, k: ElementKey, on: bool) {
        if let Some(e) = self.el_mut(k) {
            e.hover_messages = on;
        }
    }

    /// Changes the tooltip preferences (§6.1).
    pub fn set_tooltip_preferences(&mut self, enable: bool, delay: f32) {
        let enable_changed = enable != self.settings.tooltip_enable;
        self.settings.tooltip_enable = enable;
        if delay.is_finite() {
            self.settings.tooltip_delay = delay.max(0.0);
        }
        if enable_changed {
            self.hide_tooltip();
            self.clear_hover_handled();
        }
        self.hover_check();
    }

    fn clear_hover_handled(&mut self) {
        self.tooltip.handled = false;
    }

    /// The hovered element changed (§6.2 step 2, §6.6).
    pub(crate) fn tooltip_hover_changed(&mut self) {
        self.hide_tooltip();
        self.clear_hover_handled();
    }

    /// Hides and destroys the tooltip panel and the panels shown with it (§6.6).
    pub fn hide_tooltip(&mut self) {
        // The panels go in one batch: one pass over the UI however many there are.
        let mut doomed = std::mem::take(&mut self.tooltip.extra);
        if let Some(p) = self.tooltip.panel.take()
            && self.exists(p)
        {
            if self.prop(p, self.v.pids.broadcast_on_tooltip_stop).bool() {
                self.post(p, self.v.msgs.tooltip_stopped, MsgArg::default());
            }
            doomed.push(p);
        }
        self.destroy_many(&doomed);
    }

    pub(crate) fn tooltip_forget(&mut self, k: ElementKey) {
        if self.tooltip.panel == Some(k) {
            self.tooltip.panel = None;
        }
        self.tooltip.extra.retain(|x| *x != k);
    }

    /// The hover check of the UI tick (§6.2 step 3; decision 0014 D8 step 7 item 2).
    pub fn hover_check(&mut self) {
        if self.tooltip.handled
            || self.input.capture.is_some()
            || self.input.outside
            || self.input.lock.is_some()
        {
            return;
        }
        let Some(h) = self.input.hovered.filter(|h| self.exists(*h)) else {
            return;
        };
        let hd = self.prop(h, self.v.pids.hover_delay);
        let delay = if hd.is_set() {
            hd.float()
        } else {
            self.settings.tooltip_delay
        };
        if self.now <= self.input.hover_time + f64::from(delay) {
            return;
        }
        self.hide_tooltip();
        let handled = self.hover_handler(h);
        self.tooltip.handled = handled;
    }

    /// The base hover handler (§6.2 step 4), with the examination request of
    /// `item-tooltips.md` §9: an element that asks for an examination tooltip (itself or
    /// its nearest asking ancestor) gets one requested instead of a text tooltip, and the
    /// result is "handled", while `TooltipEnable` is true.
    fn hover_handler(&mut self, h: ElementKey) -> bool {
        let mut result = false;
        if self.settings.tooltip_enable
            && let Some(t) = self.examine_target(h)
        {
            let pointer = self.input.pointer.unwrap_or((0, 0));
            self.emit(crate::events::UiEvent::ExamineHover {
                element: t,
                pointer,
            });
            return true;
        }
        if self.prop(h, self.v.pids.tooltips_on).bool() && self.settings.tooltip_enable {
            self.show_tooltip(h);
            result = true;
        }
        let marked = self
            .el(h)
            .is_some_and(|e| e.hover_messages || e.plugin.is_some());
        if marked {
            let pos = self.pointer_rel(h);
            let a = self.post(
                h,
                self.v.msgs.mouse_hover,
                MsgArg {
                    pos,
                    ..MsgArg::default()
                },
            );
            result = a.is_some_and(|a| a >= Answer::Handled);
        }
        result
    }

    /// Tries to show the tooltip of `k` (§6.3–§6.5).
    pub(crate) fn show_tooltip(&mut self, k: ElementKey) {
        let p = self.v.pids;
        // Step 1: text.
        let text = match self
            .el(k)
            .and_then(|e| e.tooltip_text.clone())
            .filter(|t| !t.is_empty())
        {
            Some(t) => t,
            None => match self.property(k, p.tooltip_entry).cloned() {
                Some(PropertyValue::StringInfo(si)) => {
                    self.env.resolve_string(&si).unwrap_or_default()
                }
                _ => String::new(),
            },
        };
        if text.is_empty() {
            return;
        }
        // Step 2.
        let id = self.prop(k, p.tooltip_id).key();
        if id == 0 {
            return;
        }
        // Step 3.
        let mut layout = self.prop(k, p.tooltip_layout).key();
        if layout == 0 {
            layout = self.el(k).map_or(0, |e| e.layout_did.0);
        }
        if layout == 0 {
            return;
        }
        // Step 4.
        self.hide_tooltip();
        let Ok(panel) = self.create_element(None, Did(layout), id) else {
            return;
        };
        if let Some(e) = self.el_mut(panel) {
            e.root_panel = true;
            e.screen_level = true;
        }
        self.tooltip.panel = Some(panel);
        // Step 5.
        let tid = self.prop(panel, p.tooltip_text_id).key();
        let text_el = (tid != 0)
            .then(|| self.find_descendant(panel, tid))
            .flatten()
            .filter(|t| self.class(*t) == Some(WidgetClass::Text));
        let Some(te) = text_el else {
            self.tooltip.panel = None;
            self.destroy(panel);
            return;
        };
        self.set_text(te, Some(text.clone()));
        // Size (§6.4).
        let max_w = self.prop(te, p.max_width);
        let wrap = if max_w.is_set() {
            max_w.int()
        } else {
            self.screen_size.0
        };
        let props = self.properties(te);
        let tb = self.layout_box(te).unwrap_or_default();
        let req = crate::env::TextRequest {
            text: &text,
            props: &props,
            size: (tb.w(), tb.h()),
            wrap_width: Some(wrap),
        };
        let extent = self.env.shape_text(&req).map(|t| t.extent);
        if self.attach_raw(panel, self.screen).is_err() {
            self.tooltip.panel = None;
            self.destroy(panel);
            return;
        }
        let pb = self.layout_box(panel).unwrap_or_default();
        let (mut w, mut h) = (pb.w(), pb.h());
        if let Some((ew, eh)) = extent {
            w = w.saturating_add(ew.saturating_sub(tb.w()));
            h = h.saturating_add(eh.saturating_sub(tb.h()));
        }
        self.set_property(
            panel,
            p.object_mode,
            PropertyValue::Enum(object_mode::CANVAS_RESIZE),
        );
        let b = self.layout_box(panel).unwrap_or_default();
        self.set_layout_box(panel, crate::geom::IBox::from_xywh(b.l, b.t, w, h));
        // Placement (§6.5).
        let (px, py) = self.input.pointer.unwrap_or((0, 0));
        let cs = self.cursor_size();
        let (sw, sh) = self.screen_size;
        let mut x = px.saturating_add(cs);
        let mut y = py.saturating_add(cs);
        x = x.max(0);
        y = y.max(0);
        x = x.min(sw.saturating_sub(w));
        y = y.min(sh.saturating_sub(h));
        self.move_to(panel, x, y);
    }

    /// The displayed cursor size (§9.3): trunc(32 × `UIScale_Cursor`).
    #[must_use]
    pub fn cursor_size(&self) -> i32 {
        let s = self.settings.scale_cursor;
        let s = if s <= 0.0 || s > 4.0 || s.is_nan() {
            1.0
        } else {
            s
        };
        trunc_f32(32.0 * s)
    }

    /// Sets the cursor scale preference (`UIScale_Cursor`, §9.3).
    pub fn set_cursor_scale(&mut self, s: f32) {
        self.settings.scale_cursor = if s <= 0.0 || s > 4.0 || s.is_nan() {
            1.0
        } else {
            s
        };
    }
}
