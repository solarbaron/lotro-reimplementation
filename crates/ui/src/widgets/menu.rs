// SPDX-License-Identifier: GPL-3.0-or-later
//! Menu (`ui-widgets.md` §11) and GroupBox (§3.4).

use tessera_formats::did::Did;
use tessera_formats::property::PropertyValue;

use crate::events::{Answer, Message, MsgArg};
use crate::geom::half;
use crate::tree::ElementKey;
use crate::ui::Ui;
use crate::widgets::WidgetClass;

impl Ui {
    /// Menu set-up at post-creation (§11.2).
    pub(crate) fn menu_post_create(&mut self, k: ElementKey) {
        let p = self.v.pids;
        let popup_id = self.prop(k, p.menu_popup).key();
        let lay = self.prop(k, p.menu_popup_layout).key();
        let lay = if lay == 0 {
            self.el(k).map_or(0, |e| e.layout_did.0)
        } else {
            lay
        };
        if popup_id == 0 || lay == 0 {
            return;
        }
        // Step 1: its own root-level element, hidden, activatable.
        let Ok(popup) = self.create_element(Some(self.screen), Did(lay), popup_id) else {
            return;
        };
        if let Some(e) = self.el_mut(popup) {
            e.root_panel = true;
            e.screen_level = true;
        }
        self.set_shown(popup, false);
        self.set_property(popup, p.activatable, PropertyValue::Bool(true));
        // Step 2.
        let lb_id = self.prop(k, p.menu_listbox).key();
        let list = (lb_id != 0)
            .then(|| self.find_descendant(popup, lb_id))
            .flatten()
            .filter(|l| self.class(*l).is_some_and(|c| c.is_list()));
        if let Some(m) = self.el_mut(k).and_then(|e| e.widget.menu.as_mut()) {
            m.popup = Some(popup);
            m.list = list;
        }
        if let Some(l) = list {
            self.attach_listener(l, k);
            let def = self.prop(k, p.menu_default_item).key();
            let entry = (def != 0)
                .then(|| {
                    self.list_entries(l)
                        .into_iter()
                        .find(|e| self.element_id(*e) == Some(def))
                })
                .flatten();
            self.list_select(l, entry, false);
            self.set_property(l, p.scrollable_notify_size, PropertyValue::Bool(true));
        }
        self.menu_update_display(k);
        // Step 3.
        let w = self.layout_box(k).map_or(0, |b| b.w());
        self.set_property(popup, p.min_width, PropertyValue::Int(w));
        self.set_property(popup, p.blocks_clicks, PropertyValue::Bool(true));
    }

    /// Updates the selection display (§11.4).
    fn menu_update_display(&mut self, k: ElementKey) {
        let p = self.v.pids;
        let id = self.prop(k, p.menu_selection_display).key();
        if id == 0 {
            return;
        }
        let Some(display) = self
            .find_descendant(k, id)
            .filter(|d| self.class(*d) == Some(WidgetClass::Text))
        else {
            return;
        };
        let list = self
            .el(k)
            .and_then(|e| e.widget.menu.as_ref())
            .and_then(|m| m.list);
        let selected = list.and_then(|l| self.list_selected(l));
        let text = match selected {
            Some(s) => {
                let t = if self.class(s) == Some(WidgetClass::Text) {
                    Some(s)
                } else {
                    self.first_text(s)
                };
                t.map(|t| self.text_of(t)).unwrap_or_default()
            }
            None => {
                let v = self.prop(k, p.menu_no_selection_text).value().cloned();
                match v {
                    Some(PropertyValue::StringInfo(si)) => {
                        self.env.resolve_string(&si).unwrap_or_default()
                    }
                    _ => String::new(),
                }
            }
        };
        self.set_text(display, Some(text));
    }

    fn first_text(&self, k: ElementKey) -> Option<ElementKey> {
        let mut stack: Vec<ElementKey> = self.children(k).into_iter().rev().collect();
        let mut n = 0;
        while let Some(c) = stack.pop() {
            n += 1;
            if n > 1 << 16 {
                break;
            }
            if self.class(c) == Some(WidgetClass::Text) {
                return Some(c);
            }
            stack.extend(self.children(c).into_iter().rev());
        }
        None
    }

    /// Opening and closing (§11.3) and selections from the popup's list (§11.4).
    pub(crate) fn menu_message(&mut self, k: ElementKey, m: &Message) -> Answer {
        let ms = self.v.msgs;
        let p = self.v.pids;
        let Some(mp) = self.el(k).and_then(|e| e.widget.menu.clone()) else {
            return Answer::NotHandled;
        };
        if m.msg == ms.listbox_new_selection && Some(m.source) == mp.list {
            self.menu_update_display(k);
            if self.prop(k, p.menu_auto_close).bool() && mp.open {
                self.menu_close(k);
            }
            return Answer::NotHandled;
        }
        if m.msg != ms.mouse_click || !self.is_ancestor_or_self(k, m.source) {
            return Answer::NotHandled;
        }
        if mp.open {
            self.menu_close(k);
            return Answer::NotHandled;
        }
        let Some(popup) = mp.popup else {
            return Answer::NotHandled;
        };
        let has_entries = mp.list.is_some_and(|l| !self.list_entries(l).is_empty());
        if !has_entries {
            return Answer::NotHandled;
        }
        let mb = self.displayed_box(k);
        let pb = self.layout_box(popup).unwrap_or_default();
        let mut x = mb.l;
        if self.prop(k, p.menu_open_center).bool() {
            x = x.saturating_sub(half(pb.w().saturating_sub(mb.w())));
        }
        let y = if self.prop(k, p.menu_open_up).bool() {
            mb.t.saturating_sub(pb.h())
        } else {
            mb.t.saturating_add(mb.h()).saturating_sub(1)
        };
        self.move_to(popup, x, y);
        self.set_shown(popup, true);
        self.bring_to_front(popup);
        if let Some(mm) = self.el_mut(k).and_then(|e| e.widget.menu.as_mut()) {
            mm.open = true;
        }
        self.set_property(k, p.button_highlighted, PropertyValue::Bool(true));
        self.post(k, ms.menu_opened, MsgArg::default());
        Answer::NotHandled
    }

    fn menu_close(&mut self, k: ElementKey) {
        let Some(mp) = self.el(k).and_then(|e| e.widget.menu.clone()) else {
            return;
        };
        if let Some(popup) = mp.popup {
            self.set_shown(popup, false);
        }
        if let Some(mm) = self.el_mut(k).and_then(|e| e.widget.menu.as_mut()) {
            mm.open = false;
        }
        self.set_property(
            k,
            self.v.pids.button_highlighted,
            PropertyValue::Bool(false),
        );
        self.post(k, self.v.msgs.menu_closed, MsgArg::default());
    }

    /// Whether a menu's popup is open.
    #[must_use]
    pub fn menu_is_open(&self, k: ElementKey) -> bool {
        self.el(k)
            .and_then(|e| e.widget.menu.as_ref())
            .is_some_and(|m| m.open)
    }

    /// The popup of a menu.
    #[must_use]
    pub fn menu_popup(&self, k: ElementKey) -> Option<ElementKey> {
        self.el(k)?.widget.menu.as_ref()?.popup
    }

    // ----- GroupBox (§3.4) ---------------------------------------------------------------

    /// Selection change of a GroupBox.
    pub(crate) fn groupbox_selected_changed(&mut self, k: ElementKey) {
        let st = self.v.states;
        let v = self.prop(k, self.v.pids.gb_selected).key();
        let recorded = self
            .el(k)
            .and_then(|e| e.widget.group_selection)
            .unwrap_or(0);
        if recorded != 0
            && let Some(old) = self.find_descendant(k, recorded)
        {
            self.request_state(old, st.normal);
        }
        if let Some(e) = self.el_mut(k) {
            e.widget.group_selection = Some(v);
        }
        if let Some(new) = self.find_descendant(k, v) {
            self.request_state(new, st.highlight);
        }
    }

    /// GroupBox completion (§3.4).
    pub(crate) fn groupbox_completion(&mut self, k: ElementKey) {
        let p = self.v.pids;
        let d = self.prop(k, p.gb_default).key();
        if d != 0 {
            self.set_property(k, p.gb_selected, PropertyValue::Enum(d));
        }
    }

    /// GroupBox clicks (§3.4).
    pub(crate) fn groupbox_message(&mut self, k: ElementKey, m: &Message) -> Answer {
        if m.msg != self.v.msgs.button_click {
            return Answer::NotHandled;
        }
        let p = self.v.pids;
        let e = m.source_id;
        let selected = self.prop(k, p.gb_selected).key();
        if e == selected && selected != 0 {
            self.request_state(m.source, self.v.states.highlight);
            let allow = self.prop(k, p.gb_allow_click_selected).bool();
            return if super::groupbox::click_continues(e, selected, allow) {
                Answer::NotHandled
            } else {
                Answer::Stop
            };
        }
        self.set_property(k, p.gb_selected, PropertyValue::Enum(e));
        Answer::NotHandled
    }
}
