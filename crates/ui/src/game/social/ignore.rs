// SPDX-License-Identifier: GPL-3.0-or-later
//! The ignore page of the Social panel (`social-panels.md` §3.6, Tessera rule §9 TR6): the
//! names of the last `IgnoreList`, sorted by name (or note) with reversal, the selection,
//! the buttons, the add dialog and the row menu.

use crate::events::UiEvent;
use crate::tree::ElementKey;
use crate::ui::Ui;

use super::super::chat::notice::{
    MAX_NAME_BYTES, TextRef, args as A, is_valid_name, name_equal, tables as T,
};
use super::vocab::SocialVocab;
use super::{
    ContextMenu, DialogPurpose, MenuEntry, SocialAction, SocialIntent, TextDialog, ensure_rows,
    find, game_message, ghost, listen_id, mouse_button, row_of,
};

/// Most ignored names kept (Tessera cap).
pub const MAX_IGNORED: usize = 1024;

/// The sort column (§3.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IgnoreColumn {
    /// The name (TR6: the initial column).
    Member,
    /// The note.
    Note,
}

/// The ignore page model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IgnoreModel {
    names: Vec<String>,
    column: IgnoreColumn,
    descending: bool,
    selected: Option<String>,
}

impl Default for IgnoreModel {
    /// TR6: name, ascending.
    fn default() -> Self {
        Self {
            names: Vec::new(),
            column: IgnoreColumn::Member,
            descending: false,
            selected: None,
        }
    }
}

impl IgnoreModel {
    /// `IgnoreList { names }`: the rows are replaced; the selection is kept by name.
    pub fn set_list(&mut self, names: &[String]) {
        self.names = names.iter().take(MAX_IGNORED).cloned().collect();
        if let Some(s) = &self.selected {
            self.selected = self.names.iter().find(|n| name_equal(n, s)).cloned();
        }
    }

    /// A click on a header: the same column flips the direction, another column becomes
    /// the sort column, ascending.
    pub fn click_header(&mut self, c: IgnoreColumn) {
        if c == self.column {
            self.descending = !self.descending;
        } else {
            self.column = c;
            self.descending = false;
        }
    }

    /// The sort column and whether it is descending.
    #[must_use]
    pub fn sort(&self) -> (IgnoreColumn, bool) {
        (self.column, self.descending)
    }

    /// The names in display order. Notes are empty (TR6), so both columns order by name,
    /// case-insensitively.
    #[must_use]
    pub fn sorted(&self) -> Vec<String> {
        let mut v = self.names.clone();
        let key = |a: &String| (a.to_lowercase(), a.clone());
        if self.descending {
            v.sort_by_cached_key(|a| std::cmp::Reverse(key(a)));
        } else {
            v.sort_by_cached_key(key);
        }
        v
    }

    /// The selected name.
    #[must_use]
    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    /// Selects a name (or clears the selection).
    pub fn select(&mut self, name: Option<&str>) {
        self.selected = name.and_then(|n| self.names.iter().find(|x| *x == n).cloned());
    }

    /// (Remove, Note) normal: Remove with a selection, Note never (TR6: no notes).
    #[must_use]
    pub fn buttons(&self) -> (bool, bool) {
        (self.selected.is_some(), false)
    }

    /// The add dialog (§3.6).
    #[must_use]
    pub fn add_dialog() -> TextDialog {
        TextDialog {
            purpose: DialogPurpose::AddIgnore,
            question: TextRef::new(T::UI_FRIENDS, 0x0903_7E74),
            max_len: MAX_NAME_BYTES,
            default: String::new(),
        }
    }

    /// The add dialog's answer: `Ignore` for a valid name, else `None` (see
    /// [`DialogPurpose::answer`] for the line an invalid name posts).
    #[must_use]
    pub fn answer_add(answer: &str) -> Option<SocialIntent> {
        let name = answer.trim();
        is_valid_name(name).then(|| SocialIntent::Ignore {
            name: name.to_string(),
        })
    }

    /// Remove: `Unignore` for the selected name.
    #[must_use]
    pub fn remove(&self) -> Option<SocialIntent> {
        self.selected
            .clone()
            .map(|name| SocialIntent::Unignore { name })
    }

    /// The row menu of `name` (§3.6; the note entry ghosted, TR6).
    #[must_use]
    pub fn menu(&self, name: &str) -> Option<ContextMenu> {
        let n = self.names.iter().find(|x| *x == name)?.clone();
        Some(ContextMenu {
            heading: Some(TextRef::new(T::UI_FRIENDS, 0x0B60_7755).text(A::IGNORE_NAME, n.clone())),
            entries: vec![
                MenuEntry {
                    label: TextRef::new(T::UI_FRIENDS, 0x0177_E995),
                    enabled: true,
                    intent: Some(SocialIntent::Unignore { name: n }),
                },
                MenuEntry {
                    label: TextRef::new(T::UI_FRIENDS, 0x0F57_CB45),
                    enabled: false,
                    intent: None,
                },
            ],
        })
    }
}

/// The ignore page on the element tree.
#[derive(Debug, Clone, PartialEq)]
pub struct IgnorePage {
    vocab: SocialVocab,
    /// `IgnorePage`.
    pub page: ElementKey,
    /// `IgnoreListBox`.
    pub list: Option<ElementKey>,
    /// The rows, in list order.
    pub rows: Vec<ElementKey>,
    member: Option<ElementKey>,
    note_header: Option<ElementKey>,
    add: Option<ElementKey>,
    remove: Option<ElementKey>,
    note: Option<ElementKey>,
    names: Vec<String>,
}

impl IgnorePage {
    /// Binds the ignore page below `root` and registers the game-code handlers.
    #[must_use]
    pub fn bind(ui: &mut Ui, vocab: &SocialVocab, root: ElementKey) -> Option<Self> {
        let ids = vocab.ids;
        let page = find(ui, root, ids.ignore_page)?;
        let f = |id: u32| find(ui, page, id);
        let p = Self {
            vocab: vocab.clone(),
            page,
            list: f(ids.ignore_list),
            rows: Vec::new(),
            member: f(ids.ignore_header_member),
            note_header: f(ids.ignore_header_note),
            add: f(ids.ignore_add),
            remove: f(ids.ignore_remove),
            note: f(ids.ignore_note),
            names: Vec::new(),
        };
        let ms = ui.vocab().msgs;
        for id in [
            ids.ignore_header_member,
            ids.ignore_header_note,
            ids.ignore_add,
            ids.ignore_remove,
            ids.ignore_note,
        ] {
            listen_id(ui, id, ms.button_click);
        }
        listen_id(ui, ids.ignore_list, ms.listbox_new_selection);
        listen_id(ui, ids.ignore_template, ms.mouse_click);
        Some(p)
    }

    /// Shows the model.
    pub fn refresh(&mut self, ui: &mut Ui, model: &IgnoreModel) {
        let Some(list) = self.list else { return };
        let ids = self.vocab.ids;
        let names = model.sorted();
        self.rows = ensure_rows(
            ui,
            list,
            self.vocab.layouts.ignore,
            ids.ignore_template,
            names.len(),
        );
        self.names = names.iter().take(self.rows.len()).cloned().collect();
        let mut sel = None;
        for (k, n) in self.rows.clone().into_iter().zip(&names) {
            if let Some(l) = find(ui, k, ids.ignore_row_name) {
                ui.set_text(l, Some(n.clone()));
            }
            if let Some(l) = find(ui, k, ids.ignore_row_note) {
                ui.set_text(l, None);
            }
            if model.selected() == Some(n.as_str()) {
                sel = Some(k);
            }
        }
        if ui.list_selected(list) != sel {
            ui.list_select(list, sel, false);
        }
        self.update_buttons(ui, model);
    }

    fn update_buttons(&self, ui: &mut Ui, model: &IgnoreModel) {
        let (remove, note) = model.buttons();
        ghost(ui, self.add, false);
        ghost(ui, self.remove, !remove);
        ghost(ui, self.note, !note);
    }

    /// Handles a UI event: headers re-sort, the selection updates the buttons, Add asks for
    /// the dialog, Remove yields `Unignore`, a right click asks for the row menu.
    pub fn handle_event(
        &mut self,
        ui: &mut Ui,
        ev: &UiEvent,
        model: &mut IgnoreModel,
    ) -> Vec<SocialAction> {
        let Some(m) = game_message(ev).cloned() else {
            return Vec::new();
        };
        let ms = ui.vocab().msgs;
        let mut out = Vec::new();
        if m.msg == ms.button_click {
            if Some(m.source) == self.member {
                model.click_header(IgnoreColumn::Member);
                self.refresh(ui, model);
            } else if Some(m.source) == self.note_header {
                model.click_header(IgnoreColumn::Note);
                self.refresh(ui, model);
            } else if Some(m.source) == self.add {
                out.push(SocialAction::Dialog(IgnoreModel::add_dialog()));
            } else if Some(m.source) == self.remove {
                out.extend(model.remove().map(SocialAction::Intent));
            }
        } else if m.msg == ms.listbox_new_selection && Some(m.source) == self.list {
            let sel = self
                .list
                .and_then(|l| ui.list_selected(l))
                .and_then(|k| self.rows.iter().position(|r| *r == k))
                .and_then(|i| self.names.get(i).cloned());
            model.select(sel.as_deref());
            self.update_buttons(ui, model);
        } else if m.msg == ms.mouse_click
            && mouse_button(&m) == 2
            && let Some(i) = row_of(ui, &self.rows, m.source)
        {
            model.select(self.names.get(i).map(String::as_str));
            if let (Some(list), Some(k)) = (self.list, self.rows.get(i).copied()) {
                ui.list_select(list, Some(k), false);
            }
            self.update_buttons(ui, model);
            if let Some(menu) = self.names.get(i).and_then(|n| model.menu(n)) {
                out.push(SocialAction::Menu(menu));
            }
        }
        out
    }
}
