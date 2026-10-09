// SPDX-License-Identifier: GPL-3.0-or-later
//! Drag-box frames, the game type `HiddenDragBox` (`game-panels.md` §4).

use crate::events::Message;
use crate::tree::ElementKey;
use crate::ui::Ui;

use super::vocab::GameVocab;
use super::{find_below, string_prop, switch_if_defined};

/// One `HiddenDragBox` (§4): three flags and the three frame children.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HiddenDragBox {
    /// The drag box.
    pub key: ElementKey,
    /// `HiddenDragBox_DragBox`.
    pub frame: Option<ElementKey>,
    /// `HiddenDragBox_DragBar`.
    pub bar: Option<ElementKey>,
    /// `HiddenDragBox_TitleText`.
    pub title: Option<ElementKey>,
    /// *shown*: the frame is shown (edit mode for this box).
    pub shown: bool,
    /// *dragging*: a mouse button went down on the box and is still held.
    pub dragging: bool,
    /// *over*: the pointer is over the drag bar.
    pub over: bool,
}

impl HiddenDragBox {
    /// Post-creation (§4.3 step 1): finds the three children, copies the resolved
    /// `HiddenDragBox_Title` into the title text and the drag bar's tooltip (Tessera rule
    /// of the §4.3 OPEN: the tooltip is the title text, shown only while the frame is
    /// shown, since the hidden bar cannot be hovered), and resets — unless the box has
    /// `UIComponent_UIScaler_Target_Element` and no element with that ID exists below it.
    pub fn post_create(ui: &mut Ui, gv: &GameVocab, key: ElementKey) -> Self {
        let ids = gv.ids;
        let mut b = Self {
            key,
            frame: find_below(ui, key, ids.drag_box_frame),
            bar: find_below(ui, key, ids.drag_box_bar),
            title: find_below(ui, key, ids.drag_box_title),
            shown: false,
            dragging: false,
            over: false,
        };
        if let Some(text) = string_prop(ui, key, gv.pids.dbox_title) {
            if let Some(t) = b.title {
                ui.set_text(t, Some(text.clone()));
            }
            if let Some(bar) = b.bar {
                ui.set_tooltip_text(bar, Some(text));
            }
        }
        let target = ui.prop(key, gv.pids.scaler_target);
        let skip = target.is_set() && find_below(ui, key, target.key()).is_none();
        if !skip {
            b.reset(ui, gv);
        }
        b
    }

    /// The state property §4.2 step 2 chooses.
    #[must_use]
    pub fn state_property(&self, gv: &GameVocab) -> u32 {
        let p = gv.pids;
        if !self.shown {
            p.dbox_hidden_state
        } else if self.dragging {
            p.dbox_dragging_state
        } else if self.over {
            p.dbox_mouseover_state
        } else {
            p.dbox_visible_state
        }
    }

    /// Refresh (§4.2): children visible = *shown*; the state the chosen property names, if
    /// the box has the property, passed on to the children (§4.5).
    pub fn refresh(&self, ui: &mut Ui, gv: &GameVocab) {
        for c in [self.frame, self.bar, self.title].into_iter().flatten() {
            ui.set_shown(c, self.shown);
        }
        let prop = ui.prop(self.key, self.state_property(gv));
        if prop.is_set() {
            let s = prop.key();
            self.switch(ui, gv, s);
        }
    }

    /// Switches the box to `s` and passes it to each direct child whose
    /// `HiddenDragBoxChild_WantsHiddenDrabBoxStateChanges` is true (§4.5; skipped where the
    /// element does not define the state; one level only, the §4.5 OPEN).
    pub fn switch(&self, ui: &mut Ui, gv: &GameVocab, s: u32) {
        switch_if_defined(ui, self.key, s);
        for c in ui.children(self.key) {
            if ui.prop(c, gv.pids.dbox_child_wants).bool() {
                switch_if_defined(ui, c, s);
            }
        }
    }

    /// Reset (§4.2): all three flags false, then refresh.
    pub fn reset(&mut self, ui: &mut Ui, gv: &GameVocab) {
        self.shown = false;
        self.dragging = false;
        self.over = false;
        self.refresh(ui, gv);
    }

    /// The `ToggleHiddenDragBoxes` action (§4.3 step 2).
    pub fn toggle(&mut self, ui: &mut Ui, gv: &GameVocab) {
        if self.shown {
            self.reset(ui, gv);
        } else {
            self.shown = true;
            self.refresh(ui, gv);
        }
    }

    /// The mouse messages of §4.4. Returns whether the message changed a flag.
    pub fn message(&mut self, ui: &mut Ui, gv: &GameVocab, m: &Message) -> bool {
        let ms = ui.vocab().msgs;
        if Some(m.source) == self.bar && m.msg == ms.mouse_over_top {
            let over = m.arg.value == 1;
            if over != self.over {
                self.over = over;
                self.refresh(ui, gv);
                return true;
            }
            return false;
        }
        if m.source == self.key && (m.msg == ms.mouse_press || m.msg == ms.mouse_release) {
            let dragging = m.msg == ms.mouse_press;
            if dragging != self.dragging {
                self.dragging = dragging;
                self.refresh(ui, gv);
                return true;
            }
        }
        false
    }
}
