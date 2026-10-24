// SPDX-License-Identifier: GPL-3.0-or-later
//! States at run time (`ui-layout.md` §9.4), property-change reactions, post-creation
//! (§9.2 step 8) and the mouse-visible type rules (`ui-behaviour.md` §4.3).

use crate::props::as_bool;
use crate::tree::ElementKey;
use crate::ui::Ui;
use crate::widgets::WidgetClass;

impl Ui {
    /// Switches an element to state `s` (`ui-layout.md` §9.4).
    pub fn switch_state(&mut self, k: ElementKey, s: u32) {
        self.switch_state_depth(k, s, 0);
    }

    fn switch_state_depth(&mut self, k: ElementKey, s: u32, depth: usize) {
        if depth > crate::ui::MAX_TREE_DEPTH {
            return;
        }
        let Some(e) = self.el(k) else { return };
        // Step 1.
        let found = if s == 0 {
            None
        } else {
            e.desc.state(s).cloned()
        };
        let target = if found.is_some() { s } else { 0 };
        // Step 2.
        if target == e.state {
            return;
        }
        // Step 3.
        let pass = match &found {
            Some(st) => st.flags.pass_to_children(),
            None => e.desc.block.flags.pass_to_children(),
        };
        if let Some(e) = self.el_mut(k) {
            e.state = target;
            self.note_paint_dirty(k);
        }
        // Step 4.
        if pass {
            let ign = self.v.pids.ignore_parent_state;
            for c in self.children(k) {
                if !self.prop(c, ign).bool() {
                    self.switch_state_depth(c, target, depth + 1);
                }
            }
        }
        // Step 5.
        self.refresh_effective(k);
        // Step 6.
        let list: std::sync::Arc<[tessera_formats::ui_layout::MediaItem]> = match &found {
            Some(st) => st.media.clone().into(),
            None => match self.el(k) {
                Some(e) => e.desc.block.media.clone().into(),
                None => return,
            },
        };
        self.start_media(k, list);
    }

    /// Post-creation (`ui-layout.md` §9.2 step 8): start the own media list, mark creation
    /// finished, switch to the default state, report every effective property, recompute
    /// mouse-visible from the type rule.
    pub(crate) fn post_create(&mut self, k: ElementKey) {
        let Some(e) = self.el(k) else { return };
        let own: std::sync::Arc<[tessera_formats::ui_layout::MediaItem]> =
            e.desc.block.media.clone().into();
        let default_state = e.desc.default_state;
        self.start_media(k, own);
        if let Some(e) = self.el_mut(k) {
            e.created = true;
        }
        self.switch_state(k, default_state);
        // Report every effective property.
        let pids: Vec<u32> = self
            .el(k)
            .map(|e| e.effective.keys().copied().collect())
            .unwrap_or_default();
        for p in pids {
            self.property_changed(k, p);
        }
        self.class_post_create(k);
        self.recompute_mouse_visible(k);
    }

    /// Reacts to a change of an effective property (§9.4 step 5): the base element's
    /// reactions, then the class's.
    pub(crate) fn property_changed(&mut self, k: ElementKey, pid: u32) {
        let p = self.v.pids;
        // A composition property of a surface owner paints nothing and shapes no text
        // ([`Ui::composition_only_property`]); the passes take it.
        let composed = self.composition_only_property(k, pid);
        if composed {
            self.mark_geometry_dirty(k);
        } else {
            self.mark_dirty(k);
        }
        let val = self.property(k, pid).cloned();
        let b = val.as_ref().is_some_and(as_bool);
        match pid {
            x if x == p.hide => {
                let hide = b;
                self.set_shown(k, !hide);
            }
            x if x == p.blocks_clicks => {
                if let Some(e) = self.el_mut(k) {
                    e.blocks_clicks = b;
                }
                self.input.hover_dirty = true;
            }
            x if x == p.context_menu => self.recompute_mouse_visible(k),
            x if x == p.draw_after_children => {
                if let Some(e) = self.el_mut(k) {
                    e.draw_after_children = b;
                }
            }
            x if x == p.text_editable || x == p.text_selectable => {
                if b && let Some(e) = self.el_mut(k) {
                    e.text.dirty = true;
                }
                self.recompute_mouse_visible(k);
            }
            x if x == p.text_auto_tooltip => self.recompute_mouse_visible(k),
            x if x == p.save_loc || x == p.save_size || x == p.save_visibility => {
                self.saved_flags_changed(k);
            }
            _ => {}
        }
        if !composed
            && let Some(e) = self.el_mut(k)
            && e.class.is_text()
        {
            e.text.dirty = true;
        }
        self.class_property_changed(k, pid);
    }

    /// The type rule of `ui-behaviour.md` §4.3 for an element.
    #[must_use]
    pub fn type_rule(&self, k: ElementKey) -> bool {
        let Some(e) = self.el(k) else { return false };
        if let Some(pd) = e.plugin.as_ref() {
            // Plugin controls: the plugin layer's flag (direct writes, LPA §13.11).
            return pd.mouse_visible;
        }
        let p = self.v.pids;
        let base = || -> bool {
            self.prop(k, p.context_menu).bool()
                || (self.prop(k, p.tooltips_on).bool() && self.has_tooltip_text(k))
                || (self.prop(k, p.container).bool() && !self.prop(k, p.disallow_drag_in).bool())
                // Tessera rule: an element that asks for an examination tooltip on hover
                // (`item-tooltips.md` §9) is hovered like one with tooltip text.
                || e.examine
        };
        let scroll_host = || -> bool {
            e.widget
                .scroll
                .as_ref()
                .is_some_and(|s| s.vbar.is_some() || s.hbar.is_some())
        };
        match e.class {
            WidgetClass::Button => {
                !self.prop(k, p.button_ghosted).bool()
                    || self.prop(k, p.button_broadcast_when_ghosted).bool()
                    || self.prop(k, p.button_click_when_ghosted).bool()
            }
            WidgetClass::Scrollbar | WidgetClass::Dragbar | WidgetClass::ResizeBar => true,
            // Tessera rule (UB §4.3 OPEN): Menu, Tree, Browser, ColorPicker always.
            WidgetClass::Menu
            | WidgetClass::Tree
            | WidgetClass::Browser
            | WidgetClass::ColorPicker => true,
            WidgetClass::Text => {
                base()
                    || scroll_host()
                    || self.prop(k, p.text_editable).bool()
                    || self.prop(k, p.text_selectable).bool()
                    || (self.prop(k, p.text_auto_tooltip).bool() && e.text.cut)
            }
            WidgetClass::ListBox => (scroll_host() || base()) && e.state != self.v.states.ghosted,
            _ => base(),
        }
    }

    /// Recomputes the mouse-visible flag from the type rule (§4.3 item 1).
    pub(crate) fn recompute_mouse_visible(&mut self, k: ElementKey) {
        let v = self.type_rule(k);
        if let Some(e) = self.el_mut(k)
            && e.mouse_visible != v
        {
            e.mouse_visible = v;
            self.input.hover_dirty = true;
        }
    }

    /// Writes the mouse-visible flag directly (§4.3 item 3: widget code, game code, the
    /// plugin layer). The next recomputation replaces it.
    pub fn set_mouse_visible_flag(&mut self, k: ElementKey, v: bool) {
        if let Some(e) = self.el_mut(k) {
            e.mouse_visible = v;
        }
        self.input.hover_dirty = true;
    }

    /// The Button state choice of `ui-layout.md` §9.7 (later rows win); the button switches
    /// only to a state it defines.
    pub(crate) fn button_choose_state(&mut self, k: ElementKey) {
        let Some(e) = self.el(k) else { return };
        let Some(bp) = e.widget.button.as_ref() else {
            return;
        };
        let p = self.v.pids;
        let st = self.v.states;
        let g = self.prop(k, p.button_ghosted).bool();
        let h = self.prop(k, p.button_highlighted).bool();
        let en = self.prop(k, p.button_rollover).bool();
        let s = self.prop(k, p.button_supports_ghosted_highlight).bool();
        let pr = bp.pressed_by.is_some();
        let o = bp.over;
        let mut chosen;
        if !g {
            chosen = if h { st.highlight } else { st.normal };
            if pr || (en && o) {
                chosen = if h {
                    st.highlight_rollover
                } else {
                    st.normal_rollover
                };
            }
            if pr && o {
                chosen = if h {
                    st.highlight_pressed
                } else {
                    st.normal_pressed
                };
            }
        } else {
            chosen = if h && s {
                st.ghosted_highlight
            } else {
                st.ghosted
            };
            if pr || (en && o) {
                chosen = if h && s {
                    st.ghosted_highlight
                } else {
                    st.ghosted_rollover
                };
            }
            if pr && o {
                chosen = st.ghosted_pressed;
            }
        }
        if e.desc.state(chosen).is_some() {
            self.switch_state(k, chosen);
        }
    }

    /// A state request to a Button (`ui-widgets.md` §7.3); other elements switch directly
    /// (`ui-layout.md` §9.4).
    pub fn request_state(&mut self, k: ElementKey, s: u32) {
        let Some(e) = self.el(k) else { return };
        if !e.class.is_button() {
            self.switch_state(k, s);
            return;
        }
        let p = self.v.pids;
        let st = self.v.states;
        let h = self.prop(k, p.button_highlighted).bool();
        let sup = self.prop(k, p.button_supports_ghosted_highlight).bool();
        let mut h_changed = false;
        let mut go_step3 = true;
        // Step 1.
        if (s == st.highlight || s == st.ghosted_highlight) && !h {
            self.set_property(
                k,
                p.button_highlighted,
                tessera_formats::property::PropertyValue::Bool(true),
            );
            h_changed = true;
            if !sup {
                go_step3 = false;
            }
        } else if s == st.normal || (s == st.ghosted && sup) {
            // Step 2.
            if h {
                self.set_property(
                    k,
                    p.button_highlighted,
                    tessera_formats::property::PropertyValue::Bool(false),
                );
                h_changed = true;
                if !sup {
                    go_step3 = false;
                }
            }
        }
        if go_step3 {
            // Step 3.
            let g = [
                st.ghosted,
                st.ghosted_highlight,
                st.ghosted_rollover,
                st.ghosted_pressed,
            ]
            .contains(&s);
            if g != self.prop(k, p.button_ghosted).bool() {
                self.set_property(
                    k,
                    p.button_ghosted,
                    tessera_formats::property::PropertyValue::Bool(g),
                );
                return;
            }
        } else {
            return;
        }
        // Step 4.
        if !h_changed {
            self.switch_state(k, s);
        }
    }
}
