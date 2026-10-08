// SPDX-License-Identifier: GPL-3.0-or-later
//! Quickslot bars, the game type `QuickslotBar` (`game-panels.md` §10).

use tessera_formats::did::Did;
use tessera_formats::property::PropertyValue;

use crate::tree::ElementKey;
use crate::ui::Ui;

use super::data::{Shortcut, Shortcuts};
use super::vocab::GameVocab;
use super::{GameError, find_below};

/// Slots a bar builds (§10.1 step 2: always 12).
pub const SLOTS: i32 = 12;

/// One quickslot of a bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    /// The `QuickslotButtonTemplate` instance.
    pub button: ElementKey,
    /// Its `UI_Shortcut_Index`.
    pub index: i32,
    /// Whether the slot's shortcut is empty.
    pub empty: bool,
}

/// One `QuickslotBar` (§10).
#[derive(Debug, Clone, PartialEq)]
pub struct QuickslotBar {
    /// The bar element.
    pub key: ElementKey,
    /// The list box named by `UIComponent_Quickslot_Listbox`.
    pub list: Option<ElementKey>,
    /// The element ID `UIComponent_Quickslot_Listbox` names (0 when absent).
    pub list_id: u32,
    /// The `quickslots` layout.
    pub layout: Option<Did>,
    /// The slots of the last build, in slot order.
    pub slots: Vec<Slot>,
    /// Whether a drag is in progress (§10.2).
    pub dragging: bool,
}

impl QuickslotBar {
    /// Finds the list box and builds the slots (§10.1).
    pub fn post_create(
        ui: &mut Ui,
        gv: &GameVocab,
        key: ElementKey,
        layout: Option<Did>,
        shortcuts: &dyn Shortcuts,
    ) -> Self {
        let lid = ui.prop(key, gv.pids.quickslot_listbox).key();
        let list = if lid == 0 {
            None
        } else {
            find_below(ui, key, lid)
        };
        let mut b = Self {
            key,
            list,
            list_id: lid,
            layout,
            slots: Vec::new(),
            dragging: false,
        };
        if let Err(e) = b.rebuild(ui, gv, shortcuts) {
            tracing::debug!("quickslot bar not built: {e}");
        }
        b
    }

    /// (Re)builds the 12 slots (§10.1): empties the list box; per slot creates the button
    /// template, sets `UI_Shortcut_Index` = offset + *i*, the key label, the content (an
    /// empty shortcut gets `QuickslotEmptyTemplate`), inserts it at index *i* and hides it
    /// when the shortcut is empty (§10.2: also during a drag; the next drag begin shows
    /// them again).
    ///
    /// # Errors
    /// [`GameError::MissingProperty`] when the bar names no list box,
    /// [`GameError::MissingElement`] when the named list box is not below the bar,
    /// [`GameError::NoLayout`] without the `quickslots` layout, or a creation error.
    pub fn rebuild(
        &mut self,
        ui: &mut Ui,
        gv: &GameVocab,
        shortcuts: &dyn Shortcuts,
    ) -> Result<(), GameError> {
        let list = self.list.ok_or(if self.list_id == 0 {
            GameError::MissingProperty(gv.pids.quickslot_listbox)
        } else {
            GameError::MissingElement(self.list_id)
        })?;
        let layout = self.layout.ok_or(GameError::NoLayout("quickslots"))?;
        for e in ui.list_entries(list) {
            ui.list_remove(list, e);
        }
        self.slots.clear();
        let offset = ui.prop(self.key, gv.pids.quickslot_offset).int();
        for i in 0..SLOTS {
            let index = offset.saturating_add(i);
            let button = ui.create_element(None, layout, gv.ids.quickslot_button)?;
            ui.set_property(button, gv.pids.shortcut_index, PropertyValue::Int(index));
            if let Some(label) = find_below(ui, button, gv.ids.quickslot_label) {
                ui.set_text(label, shortcuts.key_label(index));
            }
            let shortcut = shortcuts.shortcut(index);
            let content = match &shortcut {
                Shortcut::Empty => Some(gv.ids.quickslot_empty),
                Shortcut::Kind(k) => shortcuts.content_template(*k),
            };
            if let Some(c) = content
                && let Err(e) = ui.create_element(Some(button), layout, c)
            {
                tracing::debug!("quickslot content {c:#x} not created: {e}");
            }
            if let Err(e) = ui.list_add(list, button, None) {
                ui.destroy(button);
                return Err(e.into());
            }
            let empty = shortcut == Shortcut::Empty;
            if empty {
                ui.set_shown(button, false);
            }
            self.slots.push(Slot {
                button,
                index,
                empty,
            });
        }
        Ok(())
    }

    /// `Element_drag_global_begin` (§10.2): every empty slot is shown.
    pub fn drag_begin(&mut self, ui: &mut Ui) {
        self.dragging = true;
        for s in self.slots.iter().filter(|s| s.empty) {
            ui.set_shown(s.button, true);
        }
    }

    /// `Element_drag_global_end` (§10.2): empty slots hide again (the per-bar counter of
    /// the §10.2 OPEN is taken as zero).
    pub fn drag_end(&mut self, ui: &mut Ui) {
        self.dragging = false;
        for s in self.slots.iter().filter(|s| s.empty) {
            ui.set_shown(s.button, false);
        }
    }
}
