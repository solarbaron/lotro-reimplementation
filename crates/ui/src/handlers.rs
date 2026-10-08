// SPDX-License-Identifier: GPL-3.0-or-later
//! Dispatch from the element tree to the base element's reactions and the widget classes:
//! message handlers (`ui-behaviour.md` §12.1), property changes (`ui-layout.md` §9.4 step 5),
//! post-creation (§9.2 step 8), resizes, the completion pass (`ui-widgets.md` §3.4) and the
//! per-frame work.

use tessera_formats::property::PropertyValue;

use crate::events::{Answer, Message, MsgArg};
use crate::tree::ElementKey;
use crate::ui::Ui;
use crate::vocab::commands;
use crate::widgets::WidgetClass;

impl Ui {
    /// An element's own handler for a message delivered to it (§12.1 step 2.1).
    pub(crate) fn native_handle(&mut self, x: ElementKey, m: &Message) -> Answer {
        let Some(class) = self.class(x) else {
            return Answer::NotHandled;
        };
        let ms = self.v.msgs;
        let st = self.v.states;
        let mut ans = Answer::NotHandled;
        // Base reactions: only when x is the source.
        if m.source == x {
            if m.msg == ms.mouse_press
                && m.arg
                    .command
                    .as_ref()
                    .is_some_and(|c| &*c.name == commands::SELECT_RIGHT)
                && self.prop(x, self.v.pids.context_menu).bool()
            {
                self.post(
                    x,
                    ms.open_context_menu,
                    MsgArg {
                        command: m.arg.command.clone(),
                        ..MsgArg::default()
                    },
                );
                return Answer::Stop;
            }
            // Tessera rule (UB §5.2 OPEN): pressing an editable Text element gives it
            // focus. The press is delivered after activation (§4.6 step 3), so the active
            // panel's maps are already pushed.
            if m.msg == ms.mouse_press
                && m.arg
                    .command
                    .as_ref()
                    .is_some_and(|c| &*c.name == commands::SELECT_LEFT)
                && self.el(x).is_some_and(|e| e.plugin.is_none())
                && self.is_editable(x)
                && self.focus.focus != Some(x)
            {
                self.set_focus(Some(x));
            }
            if m.msg == ms.activated {
                let s = self.state(x).unwrap_or(0);
                if s == 0 || s == st.normal {
                    self.switch_state_if_defined(x, st.normal_activated);
                }
            } else if m.msg == ms.deactivated {
                if self.state(x) == Some(st.normal_activated) {
                    self.switch_state_if_defined(x, st.normal);
                }
            } else if m.msg == ms.set_focus {
                let s = self.state(x).unwrap_or(0);
                if m.arg.value != 0 {
                    if s == 0 || s == st.normal_activated || s == st.normal {
                        self.switch_state_if_defined(x, st.normal_focussed);
                    }
                } else if s == st.normal_focussed {
                    let active = self
                        .root_panel_of(x)
                        .is_some_and(|r| self.focus.active == Some(r));
                    let to = if active {
                        st.normal_activated
                    } else {
                        st.normal
                    };
                    self.switch_state_if_defined(x, to);
                }
            } else if m.msg == ms.drag_over {
                self.drag_feedback(x, m.arg.value != 0);
            }
        }
        let mut merge = |a: Answer| {
            if a > ans {
                ans = a;
            }
        };
        if class.is_button() {
            merge(self.button_message(x, m));
        }
        if class == WidgetClass::Scrollbar {
            let a = self.scrollbar_message(x, m);
            if a == Answer::Stop {
                return a;
            }
            merge(a);
        }
        if class.is_scrollable() {
            let a = self.scrollable_message(x, m);
            if a == Answer::Stop {
                return a;
            }
            merge(a);
        }
        match class {
            WidgetClass::Menu => merge(self.menu_message(x, m)),
            WidgetClass::ListBox | WidgetClass::Tree => merge(self.list_message(x, m)),
            WidgetClass::Panel => merge(self.panel_message(x, m)),
            WidgetClass::GroupBox => {
                let a = self.groupbox_message(x, m);
                if a == Answer::Stop {
                    return a;
                }
                merge(a);
            }
            WidgetClass::Dragbar | WidgetClass::ResizeBar => merge(self.wm_message(x, m)),
            _ => {}
        }
        ans
    }

    /// A listener attached to `target` (§12.1 step 2.2).
    pub(crate) fn listener_handle(
        &mut self,
        l: ElementKey,
        target: ElementKey,
        m: &Message,
    ) -> Answer {
        let Some(class) = self.class(l) else {
            return Answer::NotHandled;
        };
        let mut ans = Answer::NotHandled;
        if class.is_scrollable() {
            ans = ans.max(self.scrollable_bar_message(l, target, m));
        }
        if class == WidgetClass::Menu {
            ans = ans.max(self.menu_message(l, m));
        }
        ans
    }

    /// Switches only when the element defines the state (`ui-behaviour.md` §5.1 step 5).
    pub(crate) fn switch_state_if_defined(&mut self, k: ElementKey, s: u32) {
        if self.el(k).is_some_and(|e| e.desc.state(s).is_some()) {
            self.switch_state(k, s);
        }
    }

    /// The nearest root panel at or above `k`.
    #[must_use]
    pub fn root_panel_of(&self, k: ElementKey) -> Option<ElementKey> {
        let mut cur = Some(k);
        let mut n = 0;
        while let Some(c) = cur {
            if self.el(c).is_some_and(|e| e.root_panel) {
                return Some(c);
            }
            n += 1;
            if n > crate::ui::MAX_TREE_DEPTH * 4 {
                return None;
            }
            cur = self.parent(c);
        }
        None
    }

    /// The class's post-creation work.
    pub(crate) fn class_post_create(&mut self, k: ElementKey) {
        match self.class(k) {
            Some(WidgetClass::Meter) => self.meter_post_create(k),
            Some(WidgetClass::Menu) => self.menu_post_create(k),
            _ => {}
        }
        if let Some(e) = self.el_mut(k)
            && e.class.is_text()
        {
            e.text.dirty = true;
        }
    }

    /// The class's reaction to a property change.
    pub(crate) fn class_property_changed(&mut self, k: ElementKey, pid: u32) {
        let Some(class) = self.class(k) else { return };
        let p = self.v.pids;
        if class.is_button()
            && [
                p.button_ghosted,
                p.button_highlighted,
                p.button_rollover,
                p.button_supports_ghosted_highlight,
            ]
            .contains(&pid)
        {
            if pid == p.button_ghosted {
                self.recompute_mouse_visible(k);
            }
            self.button_choose_state(k);
        }
        if pid == p.object_mode || pid == p.object_opacity || pid == p.keep_object_when_hidden {
            self.painter.touched = true;
            self.mark_dirty(k);
        }
        match class {
            WidgetClass::Meter => {
                if pid == p.meter_position {
                    self.meter_position_changed(k);
                } else if pid == p.meter_goal {
                    self.meter_goal_changed(k);
                } else if pid == p.meter_child_image || pid == p.meter_child_drawmode {
                    let img = self.prop(k, p.meter_child_image).key();
                    let mode = self.prop(k, p.meter_child_drawmode).key();
                    let fi = self
                        .el(k)
                        .and_then(|e| e.widget.meter.as_ref())
                        .and_then(|m| m.fill_image);
                    if let Some(fi) = fi
                        && mode != 0
                        && img != 0
                    {
                        self.set_base_did(fi, img, mode, true);
                    }
                } else if [
                    p.meter_direction,
                    p.meter_move_fill,
                    p.meter_offset,
                    p.meter_frame_array,
                    p.meter_draw_mode,
                ]
                .contains(&pid)
                {
                    self.meter_update(k);
                }
            }
            WidgetClass::Scrollbar => {
                if pid == p.sb_position {
                    self.scrollbar_position_changed(k);
                } else if pid == p.sb_goal {
                    self.scrollbar_goal_changed(k);
                } else if pid == p.sb_page_interval {
                    let v = self.prop(k, pid).float();
                    self.set_property(k, p.button_hot_interval, PropertyValue::Float(v));
                } else if pid == p.sb_page_first_interval {
                    let v = self.prop(k, pid).float();
                    self.set_property(k, p.button_hot_first, PropertyValue::Float(v));
                } else if [
                    p.sb_horizontal,
                    p.sb_proportional,
                    p.sb_widget_size,
                    p.sb_min_widget_size,
                    p.sb_disabled,
                    p.sb_hide_when_disabled,
                    p.sb_ghost_when_disabled,
                ]
                .contains(&pid)
                {
                    self.scrollbar_layout(k);
                }
            }
            WidgetClass::ListBox | WidgetClass::Tree => {
                if pid == p.lb_templates {
                    self.list_autofill(k);
                } else if [
                    p.lb_horizontal,
                    p.lb_max_columns,
                    p.lb_max_rows,
                    p.lb_only_visible,
                    p.lb_reverse,
                    p.lb_flipped,
                    p.lb_wrap_indent,
                    p.lb_ignore_cell,
                ]
                .contains(&pid)
                {
                    if let Some(l) = self.el_mut(k).and_then(|e| e.widget.list.as_mut()) {
                        l.dirty = true;
                    }
                    self.list_layout_if_dirty(k);
                }
            }
            WidgetClass::Panel if pid == p.panel_pages => self.panel_setup(k),
            WidgetClass::GroupBox if pid == p.gb_selected => self.groupbox_selected_changed(k),
            _ => {}
        }
        if class.is_scrollable()
            && (pid == p.scrollable_vbar || pid == p.scrollable_hbar)
            && self.is_created(k)
        {
            let vertical = pid == p.scrollable_vbar;
            self.scroll_update_bar(k, vertical);
            let id = self.prop(k, pid).key();
            let my_id = self.element_id(k).unwrap_or(0);
            let found = (id != 0)
                .then(|| self.find_nearest(k, id))
                .flatten()
                .filter(|b| {
                    self.class(*b) == Some(WidgetClass::Scrollbar)
                        && self.find_nearest(*b, my_id) == Some(k)
                });
            if let Some(s) = self.el_mut(k).and_then(|e| e.widget.scroll.as_mut()) {
                if vertical {
                    s.vbar = found;
                } else {
                    s.hbar = found;
                }
            }
            if let Some(b) = found {
                self.attach_listener(b, k);
            }
        }
    }

    /// The class's reaction to a size change (after the children were laid out again).
    pub(crate) fn class_resized(&mut self, k: ElementKey) {
        let Some(class) = self.class(k) else { return };
        if class == WidgetClass::Meter {
            self.meter_update(k);
        }
        if class == WidgetClass::Scrollbar {
            self.scrollbar_layout(k);
        }
        if class.is_list()
            && let Some(l) = self.el_mut(k).and_then(|e| e.widget.list.as_mut())
        {
            l.dirty = true;
        }
        if class.is_scrollable() && self.is_created(k) {
            if let Some((_, (sx, sy))) = self.scroll_state(k) {
                let (cur_sx, cur_sy) = (sx, sy);
                self.scroll_update_bar(k, true);
                self.scroll_update_bar(k, false);
                let _ = (cur_sx, cur_sy);
            }
            if let Some(e) = self.el_mut(k)
                && e.class.is_text()
            {
                e.text.dirty = true;
            }
        }
        if class.is_list() {
            self.list_layout_if_dirty(k);
        }
    }

    /// The class's part of the completion pass (`ui-widgets.md` §3.4).
    pub(crate) fn class_completion(&mut self, k: ElementKey) {
        let Some(class) = self.class(k) else { return };
        if class == WidgetClass::GroupBox {
            self.groupbox_completion(k);
        }
        if class.is_scrollable() {
            self.scrollable_completion(k);
        }
        if class == WidgetClass::Tree {
            self.tree_completion(k);
        }
        if class == WidgetClass::Scrollbar {
            self.scrollbar_setup(k);
        }
        if class.is_button() {
            self.button_choose_state(k);
        }
    }

    /// The rest of the UI's per-frame work (decision 0014 D8 step 7 item 4): media players,
    /// auto-repeat, smooth movement, part adoption and auto-fades.
    pub fn frame_work(&mut self) {
        let keys: Vec<ElementKey> = std::mem::take(&mut self.frame_set).into_iter().collect();
        for k in keys {
            if let Some(e) = self.el_mut(k) {
                e.frame_update = false;
            }
            let mut more = false;
            if self
                .el(k)
                .is_some_and(|e| !e.media.complete && !e.media.list.is_empty())
            {
                self.run_media(k);
            }
            more |= self.button_frame(k);
            more |= self.meter_frame(k);
            more |= self.scrollbar_frame(k);
            more |= self.fade_frame(k);
            if more {
                self.want_frame(k);
            }
        }
    }
}
