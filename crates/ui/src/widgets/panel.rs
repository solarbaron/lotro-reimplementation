// SPDX-License-Identifier: GPL-3.0-or-later
//! Panel: pages and tabs (`ui-widgets.md` §10).

use crate::events::{Answer, Message, MsgArg};
use crate::props::{array_elements, as_bool, as_key, struct_field};
use crate::tree::ElementKey;
use crate::ui::Ui;

impl Ui {
    /// Set-up when `UICore_Panel_pages` is set (§10.2).
    pub(crate) fn panel_setup(&mut self, k: ElementKey) {
        let p = self.v.pids;
        let entries: Vec<(u32, u32, bool)> = self
            .prop(k, p.panel_pages)
            .value()
            .map(|v| {
                array_elements(v)
                    .iter()
                    .filter_map(|e| {
                        let tab = struct_field(&e.value, p.panel_tab).map(as_key)?;
                        let page = struct_field(&e.value, p.panel_page).map(as_key)?;
                        let open = struct_field(&e.value, p.panel_page_open).is_some_and(as_bool);
                        Some((tab, page, open))
                    })
                    .collect()
            })
            .unwrap_or_default();
        // Step 1.
        let mut tab_page: Vec<(u32, u32)> = Vec::new();
        let mut page_tab: Vec<(u32, u32)> = Vec::new();
        let mut to_open: Option<(u32, u32)> = None;
        let mut tabs_to_enable = Vec::new();
        // Step 2.
        for (tab, page, open) in entries {
            if tab_page.iter().any(|(t, _)| *t == tab) {
                continue;
            }
            tab_page.push((tab, page));
            if page_tab.iter().any(|(pg, _)| *pg == page) {
                continue;
            }
            page_tab.push((page, tab));
            if tab != page {
                if open {
                    to_open = Some((page, tab));
                }
                tabs_to_enable.push(tab);
            }
        }
        if let Some(pp) = self.el_mut(k).and_then(|e| e.widget.panel.as_mut()) {
            pp.tab_page = tab_page;
            pp.page_tab = page_tab;
            pp.current = None;
        }
        for t in tabs_to_enable {
            if let Some(te) = self.find_descendant(k, t) {
                self.set_mouse_visible_flag(te, true);
            }
        }
        // Step 3.
        if let Some((page, tab)) = to_open {
            self.panel_open(k, page, tab);
        }
    }

    /// Opens (page, tab) (§10.3).
    pub fn panel_open(&mut self, k: ElementKey, page: u32, tab: u32) {
        let Some(pp) = self.el(k).and_then(|e| e.widget.panel.clone()) else {
            return;
        };
        if let Some((cp, ct)) = pp.current
            && (cp == page || ct == tab)
        {
            return;
        }
        if let Some(x) = self.el_mut(k).and_then(|e| e.widget.panel.as_mut()) {
            x.current = Some((page, tab));
        }
        let st = self.v.states;
        for (t, pg) in pp.tab_page {
            let page_el = self.find_descendant(k, pg);
            let tab_el = self.find_descendant(k, t);
            let selected = t == tab && pg == page;
            if let Some(pe) = page_el {
                self.set_shown(pe, selected);
            }
            if let Some(te) = tab_el
                && self.state(te) != Some(st.ghosted)
            {
                self.request_state(te, if selected { st.highlight } else { st.normal });
            }
        }
        self.post(k, self.v.msgs.panel_new_page, MsgArg::default());
    }

    /// Switching by the user (§10.4).
    pub(crate) fn panel_message(&mut self, k: ElementKey, m: &Message) -> Answer {
        let ms = self.v.msgs;
        if m.msg != ms.mouse_click && m.msg != ms.activated {
            return Answer::NotHandled;
        }
        if m.source == k {
            return Answer::NotHandled;
        }
        let Some(pp) = self.el(k).and_then(|e| e.widget.panel.clone()) else {
            return Answer::NotHandled;
        };
        let tab = m.source_id;
        if let Some((_, page)) = pp.tab_page.iter().find(|(t, _)| *t == tab).copied()
            && pp.page_tab.iter().any(|(pg, t)| *pg == page && *t == tab)
        {
            self.panel_open(k, page, tab);
        }
        Answer::NotHandled
    }

    /// The current (page, tab) of a panel.
    #[must_use]
    pub fn panel_current(&self, k: ElementKey) -> Option<(u32, u32)> {
        self.el(k)?.widget.panel.as_ref()?.current
    }
}
