// SPDX-License-Identifier: GPL-3.0-or-later
//! Scrollable (`ui-widgets.md` §5) and Scrollbar (§6).

use tessera_formats::property::PropertyValue;

use crate::events::{Answer, Command, Message, MsgArg};
use crate::geom::{IBox, add, half, sub, trunc_f32, trunc_f64};
use crate::tree::ElementKey;
use crate::ui::Ui;
use crate::vocab::{commands, input_maps};
use crate::widgets::WidgetClass;

/// Scroll-bar line step for scroll hosts that are not lists or trees (Tessera rule,
/// `ui-behaviour.md` §4.7 OPEN).
pub const TEXT_LINE_STEP: i32 = 16;

/// Value-to-stop of `ui-widgets.md` §6.5 (`None` = no stop rule).
#[must_use]
pub fn value_to_stop(n: Option<i32>, no_endpoints: bool, q: f32) -> Option<i32> {
    let n = n.filter(|n| *n != 0)?;
    Some(if no_endpoints {
        trunc_f32(n as f32 * q)
    } else if n > 1 {
        trunc_f64(f64::from(n.saturating_sub(1) as f32 * q) + 0.5)
    } else {
        0
    })
}

/// Stop-to-value of `ui-widgets.md` §6.5 (−1 when the stop rule does not apply).
#[must_use]
pub fn stop_to_value(n: Option<i32>, no_endpoints: bool, i: i32) -> f32 {
    let Some(n) = n.filter(|n| *n != 0) else {
        return -1.0;
    };
    let i = i.clamp(0, n.saturating_sub(1).max(0));
    if no_endpoints {
        (i as f32 + 0.5) / n as f32
    } else if n > 1 {
        i as f32 / n.saturating_sub(1) as f32
    } else {
        0.5
    }
}

/// The track of a scroll bar (`ui-widgets.md` §6.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Track {
    /// Left.
    pub tl: i32,
    /// Top.
    pub tt: i32,
    /// Right (starts at the width).
    pub tr: i32,
    /// Bottom (starts at the height).
    pub tb: i32,
}

impl Ui {
    // ----- lookups (§2.3) ----------------------------------------------------------------

    /// The **descendant** lookup of `ui-widgets.md` §2.3.
    #[must_use]
    pub fn find_descendant(&self, k: ElementKey, id: u32) -> Option<ElementKey> {
        self.find_desc_depth(k, id, 0)
    }

    fn find_desc_depth(&self, k: ElementKey, id: u32, depth: usize) -> Option<ElementKey> {
        if depth > crate::ui::MAX_TREE_DEPTH {
            return None;
        }
        let kids = self.el(k)?.children.clone();
        for c in &kids {
            if self.element_id(*c) == Some(id) {
                return Some(*c);
            }
        }
        for c in &kids {
            if let Some(f) = self.find_desc_depth(*c, id, depth + 1) {
                return Some(f);
            }
        }
        None
    }

    /// The **direct child** lookup.
    #[must_use]
    pub fn find_child(&self, k: ElementKey, id: u32) -> Option<ElementKey> {
        self.el(k)?
            .children
            .iter()
            .copied()
            .find(|c| self.element_id(*c) == Some(id))
    }

    /// The **nearest** lookup (`ui-widgets.md` §2.3). Tessera rule (§2.3 OPEN): no lookup
    /// boundaries, and the walk ends at the nearest root panel, so a part is never found in
    /// another root panel. Without a root panel above, the walk stops below the screen root.
    #[must_use]
    pub fn find_nearest(&self, k: ElementKey, id: u32) -> Option<ElementKey> {
        if let Some(f) = self.find_descendant(k, id) {
            return Some(f);
        }
        if self.element_id(k) == Some(id) {
            return Some(k);
        }
        if k == self.screen || self.el(k).is_some_and(|e| e.root_panel) {
            return None;
        }
        let mut from = k;
        let mut cur = self.parent(k);
        let mut n = 0;
        while let Some(a) = cur {
            n += 1;
            if n > crate::ui::MAX_TREE_DEPTH * 2 || a == self.screen {
                break;
            }
            for c in self.children(a) {
                if c == from {
                    continue;
                }
                if self.element_id(c) == Some(id) {
                    return Some(c);
                }
                if let Some(f) = self.find_descendant(c, id) {
                    return Some(f);
                }
            }
            if self.element_id(a) == Some(id) {
                return Some(a);
            }
            if self.el(a).is_some_and(|e| e.root_panel) {
                break;
            }
            from = a;
            cur = self.parent(a);
        }
        None
    }

    // ----- Scrollable (§5) ---------------------------------------------------------------

    /// The scrollable's content size and offsets.
    #[must_use]
    pub fn scroll_state(&self, k: ElementKey) -> Option<((i32, i32), (i32, i32))> {
        self.el(k)
            .and_then(|e| e.widget.scroll.as_ref())
            .map(|s| (s.content, s.offset))
    }

    /// Completion pass of a scrollable (§5.1).
    pub(crate) fn scrollable_completion(&mut self, k: ElementKey) {
        let p = self.v.pids;
        let my_id = self.element_id(k).unwrap_or(0);
        for (pid, vertical) in [(p.scrollable_vbar, true), (p.scrollable_hbar, false)] {
            let id = self.prop(k, pid).key();
            if id == 0 {
                continue;
            }
            let found = self.find_nearest(k, id).filter(|b| {
                self.class(*b) == Some(WidgetClass::Scrollbar)
                    && self.find_nearest(*b, my_id) == Some(k)
            });
            if let Some(bar) = found {
                if let Some(s) = self.el_mut(k).and_then(|e| e.widget.scroll.as_mut()) {
                    if vertical {
                        s.vbar = Some(bar);
                    } else {
                        s.hbar = Some(bar);
                    }
                }
                self.attach_listener(bar, k);
                self.set_mouse_visible_flag(k, true);
            }
        }
        self.scroll_update_bar(k, true);
        self.scroll_update_bar(k, false);
    }

    /// Updates one axis's scroll bar from the scrollable (§5.2).
    pub(crate) fn scroll_update_bar(&mut self, k: ElementKey, vertical: bool) {
        let Some(s) = self.el(k).and_then(|e| e.widget.scroll.clone()) else {
            return;
        };
        let bar = if vertical { s.vbar } else { s.hbar };
        let Some(bar) = bar.filter(|b| self.exists(*b)) else {
            return;
        };
        let Some(bx) = self.layout_box(k) else { return };
        let p = self.v.pids;
        let stored_c = if vertical { s.content.1 } else { s.content.0 };
        let mut c = stored_c;
        let v = if vertical { bx.h() } else { bx.w() };
        let mut sv = if vertical { s.offset.1 } else { s.offset.0 };
        let disabled = c <= v;
        if disabled {
            c = v;
            sv = 0;
        }
        if i64::from(sv) + i64::from(v) > i64::from(c) {
            sv = c.saturating_sub(v);
        }
        let f = if c == 0 {
            1.0
        } else {
            (v as f32 / c as f32).min(1.0)
        };
        let mut q = self.prop(bar, p.sb_position).float();
        if disabled {
            q = 0.0;
        }
        self.set_property(bar, p.sb_disallow_updating, PropertyValue::Bool(true));
        self.set_property(bar, p.sb_horizontal, PropertyValue::Bool(!vertical));
        self.set_property(bar, p.sb_disabled, PropertyValue::Bool(disabled));
        self.set_property(bar, p.sb_widget_size, PropertyValue::Float(f));
        self.set_property(bar, p.sb_disallow_updating, PropertyValue::Bool(false));
        if self.prop(k, p.scrollable_preserve).bool() {
            self.set_property(bar, p.sb_position, PropertyValue::Float(q));
            let s_new = trunc_f32(sub(stored_c, v) as f32 * q);
            self.set_offsets(k, if vertical { (0, s_new) } else { (s_new, 0) });
        } else {
            self.set_offset_axis(k, vertical, sv);
            let cv = sub(c, v);
            let q2 = if (cv as f32).abs() <= 0.0002 {
                0.0
            } else {
                sv as f32 / cv as f32
            };
            self.set_property(bar, p.sb_disallow_updating, PropertyValue::Bool(true));
            self.set_property(bar, p.sb_position, PropertyValue::Float(q2));
            self.set_property(bar, p.sb_horizontal, PropertyValue::Bool(!vertical));
            self.set_property(bar, p.sb_disallow_updating, PropertyValue::Bool(false));
            self.scrollbar_layout(bar);
        }
    }

    fn set_offset_axis(&mut self, k: ElementKey, vertical: bool, v: i32) {
        if let Some(s) = self.el_mut(k).and_then(|e| e.widget.scroll.as_mut()) {
            if vertical {
                s.offset.1 = v;
            } else {
                s.offset.0 = v;
            }
        }
        self.scroll_moved(k);
    }

    fn set_offsets(&mut self, k: ElementKey, o: (i32, i32)) {
        if let Some(s) = self.el_mut(k).and_then(|e| e.widget.scroll.as_mut()) {
            s.offset = o;
        }
        self.scroll_moved(k);
    }

    /// Moving the content (§5.4): a list marks its layout dirty; every scrollable repaints.
    fn scroll_moved(&mut self, k: ElementKey) {
        if let Some(l) = self.el_mut(k).and_then(|e| e.widget.list.as_mut()) {
            l.dirty = true;
        }
        self.mark_dirty(k);
        self.list_layout_if_dirty(k);
    }

    /// Scrolls to (`x`, `y`) (§5.3). `forced` skips the clamp.
    pub fn scroll_to(&mut self, k: ElementKey, x: i32, y: i32, forced: bool) {
        let Some(s) = self.el(k).and_then(|e| e.widget.scroll.clone()) else {
            return;
        };
        let Some(bx) = self.layout_box(k) else { return };
        let (mut x, mut y) = (x, y);
        if !forced && self.prop(k, self.v.pids.scrollable_keep_in_bounds).bool() {
            x = x.min(s.content.0.saturating_sub(bx.w()));
            if x <= 0 {
                x = 0;
            }
            y = y.min(s.content.1.saturating_sub(bx.h()));
            if y <= 0 {
                y = 0;
            }
        }
        if (x, y) == s.offset {
            return;
        }
        if let Some(sp) = self.el_mut(k).and_then(|e| e.widget.scroll.as_mut()) {
            sp.offset = (x, y);
        }
        self.scroll_sync_bars(k);
        self.scroll_moved(k);
    }

    /// Synchronises both bars from the offsets (§5.2 step 7, second branch).
    fn scroll_sync_bars(&mut self, k: ElementKey) {
        let Some(s) = self.el(k).and_then(|e| e.widget.scroll.clone()) else {
            return;
        };
        let Some(bx) = self.layout_box(k) else { return };
        let p = self.v.pids;
        for (bar, vertical) in [(s.vbar, true), (s.hbar, false)] {
            let Some(bar) = bar.filter(|b| self.exists(*b)) else {
                continue;
            };
            let (c, v, sv) = if vertical {
                (s.content.1.max(bx.h()), bx.h(), s.offset.1)
            } else {
                (s.content.0.max(bx.w()), bx.w(), s.offset.0)
            };
            let cv = sub(c, v);
            let q = if (cv as f32).abs() <= 0.0002 {
                0.0
            } else {
                sv as f32 / cv as f32
            };
            self.set_property(bar, p.sb_disallow_updating, PropertyValue::Bool(true));
            self.set_property(bar, p.sb_position, PropertyValue::Float(q));
            self.set_property(bar, p.sb_horizontal, PropertyValue::Bool(!vertical));
            self.set_property(bar, p.sb_disallow_updating, PropertyValue::Bool(false));
            self.scrollbar_layout(bar);
        }
    }

    /// Line and page steps (§5.6; Tessera 16 px lines for other scroll hosts).
    fn scroll_step(&self, k: ElementKey, vertical: bool, page: bool) -> i32 {
        let Some(e) = self.el(k) else { return 0 };
        let bx = e.bx;
        let v = if vertical { bx.h() } else { bx.w() };
        if page {
            return v;
        }
        match (&e.widget.list, &e.widget.scroll) {
            (Some(l), Some(s)) => {
                let n = if vertical { l.grid.1 } else { l.grid.0 };
                let c = if vertical { s.content.1 } else { s.content.0 };
                if n == 0 { 0 } else { (c / n).min(v) }
            }
            _ => TEXT_LINE_STEP.min(v.max(0)),
        }
    }

    /// A scrollable's reaction to messages from its own scroll bars (§5.5), delivered to it
    /// as a listener of the bar.
    pub(crate) fn scrollable_bar_message(
        &mut self,
        k: ElementKey,
        bar: ElementKey,
        m: &Message,
    ) -> Answer {
        let Some(s) = self.el(k).and_then(|e| e.widget.scroll.clone()) else {
            return Answer::NotHandled;
        };
        let vertical = if s.vbar == Some(bar) {
            true
        } else if s.hbar == Some(bar) {
            false
        } else {
            return Answer::NotHandled;
        };
        if m.source != bar {
            return Answer::NotHandled;
        }
        let ms = self.v.msgs;
        let step = |inc: bool, page: bool, ui: &Ui| {
            let st = ui.scroll_step(k, vertical, page);
            if inc { st } else { -st }
        };
        let delta = match m.msg {
            x if x == ms.sb_line_increment => Some(step(true, false, self)),
            x if x == ms.sb_line_decrement => Some(step(false, false, self)),
            x if x == ms.sb_page_increment => Some(step(true, true, self)),
            x if x == ms.sb_page_decrement => Some(step(false, true, self)),
            _ => None,
        };
        if let Some(d) = delta {
            let (ox, oy) = s.offset;
            if vertical {
                self.scroll_to(k, ox, oy.saturating_add(d), false);
            } else {
                self.scroll_to(k, ox.saturating_add(d), oy, false);
            }
            return Answer::Handled;
        }
        if m.msg == ms.sb_new_position {
            let q = self.prop(bar, self.v.pids.sb_position).float();
            let Some(bx) = self.layout_box(k) else {
                return Answer::Handled;
            };
            let (c, v) = if vertical {
                (s.content.1, bx.h())
            } else {
                (s.content.0, bx.w())
            };
            let off = trunc_f32(sub(c, v) as f32 * q);
            self.set_offset_axis(k, vertical, off);
            return Answer::Handled;
        }
        Answer::NotHandled
    }

    /// A scrollable's handling of messages from itself and its descendants: the wheel and
    /// the hover push (§5.7, `ui-behaviour.md` §4.7).
    pub(crate) fn scrollable_message(&mut self, k: ElementKey, m: &Message) -> Answer {
        let ms = self.v.msgs;
        if m.msg == ms.mouse_scroll {
            let Some(cmd) = m.arg.command.as_ref() else {
                return Answer::NotHandled;
            };
            if &*cmd.name != commands::SCROLL_UP && &*cmd.name != commands::SCROLL_DOWN {
                return Answer::NotHandled;
            }
            let Some(bar) = self
                .el(k)
                .and_then(|e| e.widget.scroll.as_ref())
                .and_then(|s| s.vbar)
            else {
                return Answer::NotHandled;
            };
            let usable = self.exists(bar)
                && self.is_visible(bar)
                && self.state(bar) != Some(self.v.states.ghosted)
                && !self.prop(bar, self.v.pids.sb_disabled).bool();
            if !usable {
                return Answer::NotHandled;
            }
            let cmd = cmd.clone();
            return if self.scrollbar_wheel(bar, &cmd) {
                Answer::Stop
            } else {
                Answer::NotHandled
            };
        }
        if m.msg == ms.mouse_over_top {
            let hovered = m.source;
            if m.arg.value != 0 {
                self.band_push_map(hovered, input_maps::SCROLLABLE_MOUSE, 0);
                if let Some(s) = self.el_mut(k).and_then(|e| e.widget.scroll.as_mut())
                    && !s.hover_pushes.contains(&hovered)
                {
                    s.hover_pushes.push(hovered);
                }
            } else {
                let had = self
                    .el_mut(k)
                    .and_then(|e| e.widget.scroll.as_mut())
                    .map(|s| {
                        let had = s.hover_pushes.contains(&hovered);
                        s.hover_pushes.retain(|h| *h != hovered);
                        had
                    })
                    .unwrap_or(false);
                if had {
                    self.band_pop_map(hovered, input_maps::SCROLLABLE_MOUSE);
                }
            }
        }
        Answer::NotHandled
    }

    // ----- Scrollbar (§6) ----------------------------------------------------------------

    fn bar_vertical(&self, bar: ElementKey) -> bool {
        !self.prop(bar, self.v.pids.sb_horizontal).bool()
    }

    fn bar_stops(&self, bar: ElementKey) -> (Option<i32>, bool) {
        let p = self.v.pids;
        let n = self.prop(bar, p.sb_num_stops);
        (
            n.is_set().then(|| n.int()),
            self.prop(bar, p.sb_no_endpoints).bool(),
        )
    }

    /// A part button of the bar (§6.3 "exists as a part of this bar").
    fn bar_button(&self, bar: ElementKey, pid: u32) -> Option<ElementKey> {
        let id = self.prop(bar, pid).key();
        if id == 0 {
            return None;
        }
        let bar_id = self.element_id(bar)?;
        let b = self.find_nearest(bar, id)?;
        (self.class(b).is_some_and(|c| c.is_button()) && self.find_nearest(b, bar_id) == Some(bar))
            .then_some(b)
    }

    /// The track (§6.3); moves the arrow buttons to their places.
    pub(crate) fn scrollbar_track(&mut self, bar: ElementKey) -> Track {
        let bx = self.layout_box(bar).unwrap_or_default();
        let mut t = Track {
            tl: 0,
            tt: 0,
            tr: bx.w(),
            tb: bx.h(),
        };
        let p = self.v.pids;
        if let Some(d) = self.bar_button(bar, p.sb_dec_button) {
            let db = self.layout_box(d).unwrap_or_default();
            t.tl = add(t.tl, db.w());
            t.tt = add(t.tt, db.h());
            self.move_to(d, 0, 0);
        }
        if let Some(i) = self.bar_button(bar, p.sb_inc_button) {
            let ib = self.layout_box(i).unwrap_or_default();
            t.tr = sub(t.tr, ib.w());
            t.tb = sub(t.tb, ib.h());
            self.move_to(i, t.tr, t.tb);
        }
        t
    }

    /// Lays the bar out (§6.4).
    pub(crate) fn scrollbar_layout(&mut self, bar: ElementKey) {
        let p = self.v.pids;
        if self.class(bar) != Some(WidgetClass::Scrollbar) {
            return;
        }
        // Step 1.
        if self.prop(bar, p.sb_disallow_updating).bool() {
            return;
        }
        // Step 2.
        let q = self.prop(bar, p.sb_position).float();
        // Step 3.
        let (n, no_end) = self.bar_stops(bar);
        if n.is_some_and(|n| n != 0) && !self.prop(bar, p.sb_current_stop).is_set() {
            self.scrollbar_set_from_user(bar, q);
            return;
        }
        let vertical = self.bar_vertical(bar);
        let track = self.scrollbar_track(bar);
        let bx = self.layout_box(bar).unwrap_or_default();
        let thumb = self.find_child(bar, self.v.ids.widget);
        let l = if vertical {
            sub(track.tb, track.tt)
        } else {
            sub(track.tr, track.tl)
        };
        let mut centre = (0, 0);
        if let Some(th) = thumb {
            let tb = self.layout_box(th).unwrap_or_default();
            let (mut tw, mut thh) = (tb.w(), tb.h());
            if self.prop(bar, p.sb_proportional).bool() {
                let ws = self.prop(bar, p.sb_widget_size).float();
                let mut kk = trunc_f64(f64::from(l as f32 * ws) + 0.5);
                kk = kk.max(self.prop(bar, p.sb_min_widget_size).int());
                if vertical {
                    thh = kk;
                } else {
                    tw = kk;
                }
                self.resize_layout(th, tw, thh);
            }
            let len = if vertical { thh } else { tw };
            let v = if no_end { l } else { sub(l, len) };
            let mut a = add(
                trunc_f32(v as f32 * q),
                if vertical { track.tt } else { track.tl },
            );
            if no_end {
                a = sub(a, half(len));
            }
            if vertical {
                self.move_to(th, 0, a);
            } else {
                self.move_to(th, a, 0);
            }
            if let Some(sp) = self.el_mut(bar).and_then(|e| e.widget.scrollbar.as_mut()) {
                sp.thumb_span = (a, len);
            }
            let (x, y) = if vertical { (0, a) } else { (a, 0) };
            centre = (add(x, half(tw)), add(y, half(thh)));
        }
        // Step 5: fill parts.
        let disabled = self.prop(bar, p.sb_disabled).bool();
        let ids = self.v.ids;
        let (w, h) = (bx.w(), bx.h());
        let (cx, cy) = centre;
        for (id, b) in [
            (ids.sb_left_fill, IBox::from_xywh(0, 0, cx, h)),
            (ids.sb_right_fill, IBox::from_xywh(cx, 0, sub(w, cx), h)),
            (ids.sb_top_fill, IBox::from_xywh(0, 0, w, cy)),
            (ids.sb_bottom_fill, IBox::from_xywh(0, cy, w, sub(h, cy))),
        ] {
            if let Some(f) = self.find_child(bar, id) {
                self.set_shown(f, !disabled);
                self.set_layout_box(f, b);
            }
        }
        // Step 6.
        let hide = disabled && self.prop(bar, p.sb_hide_when_disabled).bool();
        self.set_shown(bar, !hide);
        // Step 7.
        if self.prop(bar, p.sb_ghost_when_disabled).bool() {
            let s = if disabled {
                self.v.states.ghosted
            } else {
                self.v.states.normal
            };
            self.request_state(bar, s);
        }
    }

    /// Resizes an element's layout box directly (a part placed by its widget).
    pub(crate) fn resize_layout(&mut self, k: ElementKey, w: i32, h: i32) {
        let Some(b) = self.layout_box(k) else { return };
        self.set_layout_box(k, IBox::from_xywh(b.l, b.t, w, h));
    }

    /// Position property change (§6.5).
    pub(crate) fn scrollbar_position_changed(&mut self, bar: ElementKey) {
        let p = self.v.pids;
        let req = self.prop(bar, p.sb_position).float();
        let (n, no_end) = self.bar_stops(bar);
        let mut q = req;
        if let Some(i) = value_to_stop(n, no_end, q) {
            q = stop_to_value(n, no_end, i);
        }
        q = if q.is_nan() { 0.0 } else { q.clamp(0.0, 1.0) };
        if (q - req).abs() > 0.0002 || req.is_nan() {
            self.set_property(bar, p.sb_position, PropertyValue::Float(q));
        } else {
            self.scrollbar_layout(bar);
        }
    }

    /// Sets the position from the user (§6.5).
    pub fn scrollbar_set_from_user(&mut self, bar: ElementKey, value: f32) {
        let v = if value.is_nan() {
            0.0
        } else {
            value.clamp(0.0, 1.0)
        };
        let (n, no_end) = self.bar_stops(bar);
        if let Some(i) = value_to_stop(n, no_end, v) {
            self.scrollbar_select_stop(bar, i);
            return;
        }
        self.set_property(bar, self.v.pids.sb_position, PropertyValue::Float(v));
        self.post(
            bar,
            self.v.msgs.sb_new_position,
            MsgArg {
                value: i64::from(trunc_f32(v * 1000.0)),
                value2: -1,
                ..MsgArg::default()
            },
        );
    }

    /// Select stop *i* (§6.5).
    pub fn scrollbar_select_stop(&mut self, bar: ElementKey, i: i32) {
        let (n, no_end) = self.bar_stops(bar);
        let q = stop_to_value(n, no_end, i);
        if q < 0.0 {
            return;
        }
        let i = i.clamp(0, n.unwrap_or(1).saturating_sub(1).max(0));
        let p = self.v.pids;
        self.set_property(bar, p.sb_current_stop, PropertyValue::Int(i));
        self.set_property(bar, p.sb_position, PropertyValue::Float(q));
        self.post(
            bar,
            self.v.msgs.sb_new_position,
            MsgArg {
                value: i64::from(trunc_f32(q * 1000.0)),
                value2: i64::from(i),
                ..MsgArg::default()
            },
        );
    }

    /// The wheel step of a bar (§6.8). Returns whether it acted (`ui-behaviour.md` §4.7).
    pub(crate) fn scrollbar_wheel(&mut self, bar: ElementKey, cmd: &Command) -> bool {
        let p = self.v.pids;
        let vertical = self.bar_vertical(bar);
        if !vertical && !self.prop(bar, p.sb_hwheel).bool() {
            return false;
        }
        if !self.prop(bar, p.sb_position).is_set() {
            return true;
        }
        let down = &*cmd.name == commands::SCROLL_DOWN;
        if self.prop(bar, p.sb_current_stop).is_set() {
            let cur = self.prop(bar, p.sb_current_stop).int();
            let step = match (vertical, down) {
                (true, true) | (false, false) => 1,
                _ => -1,
            };
            self.scrollbar_select_stop(bar, cur.saturating_add(step));
        } else {
            let msg = if down {
                self.v.msgs.sb_line_increment
            } else {
                self.v.msgs.sb_line_decrement
            };
            self.post(bar, msg, MsgArg::value(if down { 1 } else { -1 }));
        }
        true
    }

    /// Makes a button an auto-repeat button (§6.2).
    fn make_hot(&mut self, b: ElementKey) {
        let p = self.v.pids;
        self.set_property(b, p.button_hot, PropertyValue::Bool(true));
        if !self.prop(b, p.button_hot_first).is_set() {
            self.set_property(b, p.button_hot_first, PropertyValue::Float(0.5));
        }
        if !self.prop(b, p.button_hot_interval).is_set() {
            self.set_property(b, p.button_hot_interval, PropertyValue::Float(0.125));
        }
    }

    /// Finds or adopts the arrow buttons (§6.2) and sets up the slider mode (§6.7).
    pub(crate) fn scrollbar_setup(&mut self, bar: ElementKey) {
        let p = self.v.pids;
        let mut missing = false;
        for pid in [p.sb_dec_button, p.sb_inc_button] {
            let id = self.prop(bar, pid).key();
            if id == 0 {
                continue;
            }
            match self.find_descendant(bar, id) {
                Some(b) => self.make_hot(b),
                None => missing = true,
            }
        }
        if let Some(sp) = self.el_mut(bar).and_then(|e| e.widget.scrollbar.as_mut()) {
            sp.adopt_pending = missing;
        }
        if missing {
            self.want_frame(bar);
        }
        let slider = self.prop(bar, p.sb_slider).bool();
        if slider {
            if let Some(th) = self.find_child(bar, self.v.ids.widget) {
                self.set_mouse_visible_flag(th, false);
            }
            self.set_property(bar, p.button_hot, PropertyValue::Bool(false));
        } else {
            self.make_hot(bar);
        }
        self.scrollbar_layout(bar);
    }

    /// Per-frame work of a bar: adopting arrow buttons (§6.2) and smooth movement (§6.9).
    /// Returns whether it needs more frames.
    pub(crate) fn scrollbar_frame(&mut self, bar: ElementKey) -> bool {
        let Some(sp) = self.el(bar).and_then(|e| e.widget.scrollbar.clone()) else {
            return false;
        };
        let mut more = false;
        if sp.adopt_pending {
            let p = self.v.pids;
            for pid in [p.sb_dec_button, p.sb_inc_button] {
                let id = self.prop(bar, pid).key();
                if id == 0 || self.find_descendant(bar, id).is_some() {
                    continue;
                }
                if let Some(parent) = self.parent(bar)
                    && let Some(b) = self.find_descendant(parent, id)
                    && !self.is_ancestor_or_self(b, bar)
                    && self.attach_raw(b, bar).is_ok()
                {
                    self.make_hot(b);
                }
            }
            if let Some(s) = self.el_mut(bar).and_then(|e| e.widget.scrollbar.as_mut()) {
                s.adopt_pending = false;
            }
            self.scrollbar_layout(bar);
        }
        if let Some((start, goal, t0, t1)) = sp.anim {
            let t = if t1 - t0 <= 0.0 {
                0.0
            } else {
                ((self.now - t0) / (t1 - t0)) as f32
            };
            let t = if t >= 1.0 { 1.0 } else { t };
            if t >= 1.0 {
                if let Some(s) = self.el_mut(bar).and_then(|e| e.widget.scrollbar.as_mut()) {
                    s.anim = None;
                }
            } else {
                more = true;
            }
            let q = start + (goal - start) * t;
            self.set_property(bar, self.v.pids.sb_position, PropertyValue::Float(q));
        }
        more
    }

    /// Goal change (§6.9).
    pub(crate) fn scrollbar_goal_changed(&mut self, bar: ElementKey) {
        let p = self.v.pids;
        let goal = self.prop(bar, p.sb_goal).float();
        if self.prop(bar, p.sb_smooth).bool() {
            let d = self.prop(bar, p.sb_smooth_duration);
            if d.is_set() && d.float().abs() > 0.0002 {
                let start = self.prop(bar, p.sb_position).float();
                let now = self.now;
                let dur = f64::from(d.float());
                if let Some(s) = self.el_mut(bar).and_then(|e| e.widget.scrollbar.as_mut()) {
                    s.anim = Some((start, goal, now, now + dur));
                }
                self.want_frame(bar);
            }
        } else {
            self.set_property(bar, p.sb_position, PropertyValue::Float(goal));
        }
    }

    /// The pointer position relative to an element's displayed box.
    pub(crate) fn pointer_rel(&self, k: ElementKey) -> (i32, i32) {
        let (px, py) = self.input.pointer.unwrap_or((0, 0));
        let b = self.displayed_box(k);
        (sub(px, b.l), sub(py, b.t))
    }

    /// Jump to the pointer (§6.7 step 4) with the point (`x`, `y`) in bar coordinates.
    fn scrollbar_jump(&mut self, bar: ElementKey, x: i32, y: i32) {
        let track = self.scrollbar_track(bar);
        let no_end = self.bar_stops(bar).1;
        let vertical = self.bar_vertical(bar);
        let (tw, th) = self
            .find_child(bar, self.v.ids.widget)
            .and_then(|t| self.layout_box(t))
            .map(|b| (b.w(), b.h()))
            .unwrap_or((0, 0));
        let mut px = sub(x, track.tl);
        let mut py = sub(y, track.tt);
        if !no_end {
            px = sub(px, half(tw));
            py = sub(py, half(th));
            px = px.clamp(0, sub(sub(sub(track.tr, track.tl), tw), 1).max(0));
            py = py.clamp(0, sub(sub(sub(track.tb, track.tt), th), 1).max(0));
        }
        let travel = if vertical {
            sub(sub(track.tb, track.tt), if no_end { 0 } else { th })
        } else {
            sub(sub(track.tr, track.tl), if no_end { 0 } else { tw })
        };
        let d = sub(travel, 1);
        let pos = if vertical { py } else { px };
        let value = if d > 1 { pos as f32 / d as f32 } else { 0.0 };
        self.scrollbar_set_from_user(bar, value);
    }

    fn holds_left(&self, k: ElementKey) -> bool {
        self.el(k).is_some_and(|e| {
            e.pressed
                .iter()
                .any(|c| &**c == commands::SELECT_LEFT || &**c == commands::SELECT_DBL_LEFT)
        })
    }

    /// The bar's handling of messages from itself and its parts (§6.6–§6.9).
    pub(crate) fn scrollbar_message(&mut self, bar: ElementKey, m: &Message) -> Answer {
        let ms = self.v.msgs;
        let p = self.v.pids;
        let slider = self.prop(bar, p.sb_slider).bool();
        let thumb = self.find_child(bar, self.v.ids.widget);
        let is_drag_source = (Some(m.source) == thumb && !slider) || (slider && m.source == bar);
        // Hover (§6.9).
        if m.msg == ms.mouse_over_top && m.source == bar {
            if m.arg.value != 0 {
                self.band_push_map(bar, input_maps::SCROLLABLE_MOUSE, 0);
            } else {
                self.band_pop_map(bar, input_maps::SCROLLABLE_MOUSE);
            }
            if let Some(s) = self.el_mut(bar).and_then(|e| e.widget.scrollbar.as_mut()) {
                s.pushed = m.arg.value != 0;
            }
            return Answer::NotHandled;
        }
        // Wheel on its own message (§6.8).
        if m.msg == ms.mouse_scroll && m.source == bar {
            if let Some(cmd) = m.arg.command.clone()
                && (&*cmd.name == commands::SCROLL_UP || &*cmd.name == commands::SCROLL_DOWN)
            {
                return if self.scrollbar_wheel(bar, &cmd) {
                    Answer::Stop
                } else {
                    Answer::NotHandled
                };
            }
            return Answer::NotHandled;
        }
        // Arrow buttons and track paging (§6.6).
        if m.msg == ms.button_hot_click {
            let dec = self.bar_button(bar, p.sb_dec_button);
            let inc = self.bar_button(bar, p.sb_inc_button);
            let step = if Some(m.source) == inc {
                Some(1)
            } else if Some(m.source) == dec {
                Some(-1)
            } else {
                None
            };
            if let Some(st) = step {
                if self.prop(bar, p.sb_current_stop).is_set() {
                    let cur = self.prop(bar, p.sb_current_stop).int();
                    self.scrollbar_select_stop(bar, cur.saturating_add(st));
                } else {
                    let msg = if st > 0 {
                        ms.sb_line_increment
                    } else {
                        ms.sb_line_decrement
                    };
                    self.post(bar, msg, MsgArg::value(i64::from(st)));
                }
                return Answer::Handled;
            }
            if m.source == bar && !slider {
                let (x, y) = self.pointer_rel(bar);
                let vertical = self.bar_vertical(bar);
                let pos = if vertical { y } else { x };
                let (a, len) = self
                    .el(bar)
                    .and_then(|e| e.widget.scrollbar.as_ref())
                    .map(|s| s.thumb_span)
                    .unwrap_or((0, 0));
                if pos > add(a, len) {
                    self.post(bar, ms.sb_page_increment, MsgArg::value(1));
                } else if pos < a {
                    self.post(bar, ms.sb_page_decrement, MsgArg::value(-1));
                }
                return Answer::Handled;
            }
            return Answer::NotHandled;
        }
        if !is_drag_source {
            return Answer::NotHandled;
        }
        let left = m.arg.command.as_ref().is_some_and(|c| {
            &*c.name == commands::SELECT_LEFT || &*c.name == commands::SELECT_DBL_LEFT
        });
        // Point in bar coordinates: a child's message position plus the child's position.
        let off = if m.source == bar {
            (0, 0)
        } else {
            self.layout_box(m.source)
                .map(|b| (b.l, b.t))
                .unwrap_or((0, 0))
        };
        let (x, y) = (add(m.arg.pos.0, off.0), add(m.arg.pos.1, off.1));
        if m.msg == ms.mouse_press && left {
            if slider {
                self.scrollbar_jump(bar, x, y);
            }
            let q = self.prop(bar, p.sb_position).float();
            if let Some(s) = self.el_mut(bar).and_then(|e| e.widget.scrollbar.as_mut()) {
                s.press = (x, y);
                s.drag_start = q;
                s.dragging = true;
            }
            return Answer::Handled;
        }
        if m.msg == ms.mouse_move && self.holds_left(m.source) {
            let vertical = self.bar_vertical(bar);
            let across = if vertical { x.abs() } else { y.abs() };
            let sp = self
                .el(bar)
                .and_then(|e| e.widget.scrollbar.clone())
                .unwrap_or_default();
            if across > 100 {
                if sp.dragging {
                    self.scrollbar_set_from_user(bar, sp.drag_start);
                    if let Some(s) = self.el_mut(bar).and_then(|e| e.widget.scrollbar.as_mut()) {
                        s.dragging = false;
                    }
                }
                return Answer::Handled;
            }
            if slider {
                self.scrollbar_jump(bar, x, y);
            } else {
                let track = self.scrollbar_track(bar);
                let no_end = self.bar_stops(bar).1;
                let (tw, th) = thumb
                    .and_then(|t| self.layout_box(t))
                    .map(|b| (b.w(), b.h()))
                    .unwrap_or((0, 0));
                let v = if vertical {
                    sub(sub(track.tb, track.tt), if no_end { 0 } else { th })
                } else {
                    sub(sub(track.tr, track.tl), if no_end { 0 } else { tw })
                };
                if v == 0 {
                    self.scrollbar_set_from_user(bar, 0.0);
                } else {
                    let d = if vertical {
                        sub(y, sp.press.1)
                    } else {
                        sub(x, sp.press.0)
                    };
                    self.scrollbar_set_from_user(bar, sp.drag_start + d as f32 / v as f32);
                }
            }
            if let Some(s) = self.el_mut(bar).and_then(|e| e.widget.scrollbar.as_mut()) {
                s.dragging = true;
            }
            return Answer::Handled;
        }
        if m.msg == ms.mouse_release && left {
            if let Some(s) = self.el_mut(bar).and_then(|e| e.widget.scrollbar.as_mut()) {
                s.dragging = false;
            }
            return Answer::Handled;
        }
        Answer::NotHandled
    }

    /// Sets a scrollable's content size (`ui-widgets.md` §8.3 step 4 for lists; code for
    /// other scroll hosts), updating both bars.
    pub fn set_content_size(&mut self, k: ElementKey, w: i32, h: i32) {
        let Some(old) = self
            .el(k)
            .and_then(|e| e.widget.scroll.as_ref())
            .map(|s| s.content)
        else {
            return;
        };
        if old == (w, h) {
            return;
        }
        if let Some(s) = self.el_mut(k).and_then(|e| e.widget.scroll.as_mut()) {
            s.content = (w, h);
        }
        self.scroll_update_bar(k, true);
        self.scroll_update_bar(k, false);
        if self.prop(k, self.v.pids.scrollable_notify_size).bool() {
            self.post(
                k,
                self.v.msgs.scrollable_size_changed,
                MsgArg {
                    value: i64::from(old.0),
                    value2: i64::from(old.1),
                    ..MsgArg::default()
                },
            );
        }
    }
}
