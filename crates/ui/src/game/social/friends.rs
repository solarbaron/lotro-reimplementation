// SPDX-License-Identifier: GPL-3.0-or-later
//! The friends page of the Social panel (`social-panels.md` §3.2–§3.5, Tessera rule §9
//! TR5): the rows of the last `FriendList`, the column sort with reversal, the selection
//! kept by name, the buttons' ghosting, the add dialog and the row menu.

use std::cmp::Reverse;
use std::collections::BTreeSet;

use tessera_formats::property::PropertyValue;

use crate::events::UiEvent;
use crate::props::as_argb;
use crate::tree::ElementKey;
use crate::ui::Ui;

use super::super::chat::notice::{
    MAX_NAME_BYTES, TextRef, TextSource, args as A, is_valid_name, name_equal, tables as T,
};
use super::vocab::SocialVocab;
use super::{
    ContextMenu, DialogPurpose, MenuEntry, SocialAction, SocialIntent, TextDialog, ensure_rows,
    find, game_message, ghost, listen_id, mouse_button, row_of, set_ref,
};

/// Most friend entries kept (the feed's bound, `scenario-feed.md` §6.4).
pub const MAX_FRIENDS: usize = 1024;

/// One entry of `FriendList`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FriendEntry {
    /// The name.
    pub name: String,
    /// Online.
    pub online: bool,
}

/// A sort column (§3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FriendColumn {
    /// The grouped marker.
    Grouped,
    /// The class.
    Class,
    /// The level.
    Level,
    /// The name (`Member`, the default).
    Member,
    /// The area.
    Area,
    /// The last time online.
    LastOn,
    /// The note.
    Note,
}

/// The player's group, for the row menu's invite entry (§3.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GroupState {
    /// The player is in a group.
    pub in_group: bool,
    /// The player leads it.
    pub leads: bool,
    /// The group is full (six places, §4.3).
    pub full: bool,
}

/// What one row shows (§3.3 with TR5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FriendRow {
    /// The name.
    pub name: String,
    /// Online (the name's colour: `UI_Friend_OnlineColor`, else `UI_Friend_OfflineColor`).
    pub online: bool,
    /// The grouped marker: `fellowship_fellowed` (true) or `fellowship_unknown`.
    pub grouped: bool,
    /// The class column: the unknown text (TR5).
    pub class: TextRef,
    /// The level column: the unknown text.
    pub level: TextRef,
    /// The area column: the unknown text.
    pub area: TextRef,
    /// The last-online column.
    pub last_on: TextRef,
}

/// The unknown text (UF, 0x0A8290DF).
#[must_use]
pub fn unknown_text() -> TextRef {
    TextRef::new(T::UI_FRIENDS, 0x0A82_90DF)
}

/// Which buttons are normal (true) or ghosted (false) (§3.5 with TR5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FriendButtons {
    /// Add: always.
    pub add: bool,
    /// Remove: a row is selected.
    pub remove: bool,
    /// Note: ghosted (TR5: no note field).
    pub note: bool,
    /// Tell: the selected friend is online.
    pub tell: bool,
    /// Invite: the selected friend is online and not grouped.
    pub invite: bool,
}

/// The friends page model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FriendsModel {
    entries: Vec<FriendEntry>,
    column: FriendColumn,
    descending: bool,
    selected: Option<String>,
    /// The fellowship members' names, lowercased (name-equal lookups).
    fellows: BTreeSet<String>,
}

impl Default for FriendsModel {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            column: FriendColumn::Member,
            descending: false,
            selected: None,
            fellows: BTreeSet::new(),
        }
    }
}

impl FriendsModel {
    /// `FriendList { entries }`: the rows are replaced; the selection is kept by name
    /// (TR5). At most [`MAX_FRIENDS`] entries are kept.
    pub fn set_list(&mut self, entries: &[FriendEntry]) {
        self.entries = entries.iter().take(MAX_FRIENDS).cloned().collect();
        if let Some(s) = &self.selected
            && !self.entries.iter().any(|e| &e.name == s)
        {
            self.selected = self
                .entries
                .iter()
                .find(|e| name_equal(&e.name, s))
                .map(|e| e.name.clone());
        }
    }

    /// The names of the current fellowship's members (TR5: the grouped marker). At most
    /// [`super::fellowship::MAX_MEMBERS`] names are kept.
    pub fn set_fellows(&mut self, names: &[String]) {
        self.fellows = names
            .iter()
            .take(super::fellowship::MAX_MEMBERS)
            .map(|n| n.to_lowercase())
            .collect();
    }

    fn grouped(&self, e: &FriendEntry) -> bool {
        e.online && !self.fellows.is_empty() && self.fellows.contains(&e.name.to_lowercase())
    }

    /// The sort column and whether it is descending.
    #[must_use]
    pub fn sort(&self) -> (FriendColumn, bool) {
        (self.column, self.descending)
    }

    /// A click on a header (§3.4): the same column flips the direction, another column
    /// becomes the sort column, ascending.
    pub fn click_header(&mut self, c: FriendColumn) {
        if c == self.column {
            self.descending = !self.descending;
        } else {
            self.column = c;
            self.descending = false;
        }
    }

    /// The entries in display order. The sort key of each entry (its column value and
    /// lowercased name) is computed once, not per comparison.
    #[must_use]
    pub fn sorted(&self) -> Vec<&FriendEntry> {
        let mut v: Vec<&FriendEntry> = self.entries.iter().collect();
        let key = |e: &&FriendEntry| {
            // TR5: class, level, area and note are the same (unknown or empty) for every
            // row, so these columns order by name; last online puts offline rows first
            // (they have no logout time), then by name.
            let first = match self.column {
                FriendColumn::Grouped => self.grouped(e),
                FriendColumn::LastOn => e.online,
                _ => false,
            };
            (first, e.name.to_lowercase(), e.name.clone())
        };
        if self.descending {
            v.sort_by_cached_key(|e| Reverse(key(e)));
        } else {
            v.sort_by_cached_key(key);
        }
        v
    }

    /// The rows in display order (§3.3 with TR5).
    #[must_use]
    pub fn rows(&self) -> Vec<FriendRow> {
        self.sorted()
            .into_iter()
            .map(|e| FriendRow {
                name: e.name.clone(),
                online: e.online,
                grouped: self.grouped(e),
                class: unknown_text(),
                level: unknown_text(),
                area: unknown_text(),
                last_on: if e.online {
                    TextRef::new(T::UI_FRIENDS, 0x0FE8_45CC)
                        .nested(A::ONLINE_STATE, TextRef::new(T::UI_FRIENDS, 0x0FF0_52F5))
                } else {
                    unknown_text()
                },
            })
            .collect()
    }

    /// The selected friend's name.
    #[must_use]
    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    /// Selects a friend by name (or clears the selection).
    pub fn select(&mut self, name: Option<&str>) {
        self.selected = name
            .and_then(|n| self.entries.iter().find(|e| e.name == n))
            .map(|e| e.name.clone());
    }

    fn selected_entry(&self) -> Option<&FriendEntry> {
        let s = self.selected.as_ref()?;
        self.entries.iter().find(|e| &e.name == s)
    }

    /// The buttons' states (§3.5 with TR5).
    #[must_use]
    pub fn buttons(&self) -> FriendButtons {
        let sel = self.selected_entry();
        FriendButtons {
            add: true,
            remove: sel.is_some(),
            note: false,
            tell: sel.is_some_and(|e| e.online),
            invite: sel.is_some_and(|e| e.online && !self.grouped(e)),
        }
    }

    /// The add dialog (§3.5).
    #[must_use]
    pub fn add_dialog() -> TextDialog {
        TextDialog {
            purpose: DialogPurpose::AddFriend,
            question: TextRef::new(T::UI_FRIENDS, 0x0BC2_5DB4),
            max_len: MAX_NAME_BYTES,
            default: String::new(),
        }
    }

    /// The add dialog's answer: `AddFriend` for a valid name, else `None` (see
    /// [`DialogPurpose::answer`] for the line an invalid name posts).
    #[must_use]
    pub fn answer_add(answer: &str) -> Option<SocialIntent> {
        let name = answer.trim();
        is_valid_name(name).then(|| SocialIntent::AddFriend {
            name: name.to_string(),
        })
    }

    /// Remove: `RemoveFriend` for the selected friend.
    #[must_use]
    pub fn remove(&self) -> Option<SocialIntent> {
        self.selected_entry().map(|e| SocialIntent::RemoveFriend {
            name: e.name.clone(),
        })
    }

    /// Tell (the button or a double click): a tell to the selected friend, when online.
    #[must_use]
    pub fn tell(&self) -> Option<SocialIntent> {
        self.selected_entry()
            .filter(|e| e.online)
            .map(|e| SocialIntent::StartTell {
                name: e.name.clone(),
            })
    }

    /// Invite: `InviteToFellowship` for the selected friend, when online and not grouped.
    #[must_use]
    pub fn invite(&self) -> Option<SocialIntent> {
        self.selected_entry()
            .filter(|e| e.online && !self.grouped(e))
            .map(|e| SocialIntent::InviteToFellowship {
                name: e.name.clone(),
            })
    }

    /// The row menu of friend `name` (§3.5 with TR5: note and auto-fellow ghosted).
    #[must_use]
    pub fn menu(&self, name: &str, group: GroupState) -> Option<ContextMenu> {
        let e = self.entries.iter().find(|e| e.name == name)?;
        let can_invite = (!group.in_group || (group.leads && !group.full)) && e.online;
        let entry = |token: u32, enabled: bool, intent: Option<SocialIntent>| MenuEntry {
            label: TextRef::new(T::UI_FRIENDS, token),
            enabled,
            intent: if enabled { intent } else { None },
        };
        let n = e.name.clone();
        Some(ContextMenu {
            // §3.5, SP33: the heading declares the note question's argument 0xA2D94C5.
            heading: Some(TextRef::new(T::UI_FRIENDS, 0x0193_53F5).text(A::NOTE_NAME, n.clone())),
            entries: vec![
                entry(
                    0x0743_846D,
                    e.online,
                    Some(SocialIntent::StartTell { name: n.clone() }),
                ),
                entry(
                    0x0452_9325,
                    can_invite,
                    Some(SocialIntent::InviteToFellowship { name: n.clone() }),
                ),
                entry(
                    0x033D_8545,
                    true,
                    Some(SocialIntent::RemoveFriend { name: n.clone() }),
                ),
                entry(0x044B_1505, true, Some(SocialIntent::Ignore { name: n })),
                entry(0x0386_01E5, false, None),
                entry(0x065B_E227, false, None),
            ],
        })
    }
}

/// The friends page on the element tree.
#[derive(Debug, Clone, PartialEq)]
pub struct FriendsPage {
    vocab: SocialVocab,
    /// `FriendsPage`.
    pub page: ElementKey,
    /// `FriendListBox`.
    pub list: Option<ElementKey>,
    /// The rows, in list order (row *i* shows the *i*-th sorted entry).
    pub rows: Vec<ElementKey>,
    /// (header button, column).
    headers: Vec<(ElementKey, FriendColumn)>,
    add: Option<ElementKey>,
    remove: Option<ElementKey>,
    note: Option<ElementKey>,
    invite: Option<ElementKey>,
    tell: Option<ElementKey>,
    names: Vec<String>,
}

impl FriendsPage {
    /// Binds the friends page below `root` (the Social panel or the page itself): finds its
    /// list, headers and buttons and registers the game-code handlers.
    #[must_use]
    pub fn bind(ui: &mut Ui, vocab: &SocialVocab, root: ElementKey) -> Option<Self> {
        let ids = vocab.ids;
        let page = find(ui, root, ids.friends_page)?;
        let f = |id: u32| find(ui, page, id);
        let headers: Vec<(ElementKey, FriendColumn)> = [
            (ids.friend_header_grouped, FriendColumn::Grouped),
            (ids.friend_header_class, FriendColumn::Class),
            (ids.friend_header_level, FriendColumn::Level),
            (ids.friend_header_member, FriendColumn::Member),
            (ids.friend_header_area, FriendColumn::Area),
            (ids.friend_header_last_on, FriendColumn::LastOn),
            (ids.friend_header_note, FriendColumn::Note),
        ]
        .into_iter()
        .filter_map(|(id, c)| f(id).map(|k| (k, c)))
        .collect();
        let p = Self {
            vocab: vocab.clone(),
            page,
            list: f(ids.friend_list),
            rows: Vec::new(),
            headers,
            add: f(ids.friend_add),
            remove: f(ids.friend_remove),
            note: f(ids.friend_note),
            invite: f(ids.friend_invite),
            tell: f(ids.friend_tell),
            names: Vec::new(),
        };
        let ms = ui.vocab().msgs;
        for id in [
            ids.friend_header_grouped,
            ids.friend_header_class,
            ids.friend_header_level,
            ids.friend_header_member,
            ids.friend_header_area,
            ids.friend_header_last_on,
            ids.friend_header_note,
            ids.friend_add,
            ids.friend_remove,
            ids.friend_note,
            ids.friend_invite,
            ids.friend_tell,
        ] {
            listen_id(ui, id, ms.button_click);
        }
        listen_id(ui, ids.friend_list, ms.listbox_new_selection);
        listen_id(ui, ids.friend_template, ms.mouse_dbl_click);
        listen_id(ui, ids.friend_template, ms.mouse_click);
        Some(p)
    }

    /// Shows the model: one row per entry in sort order, each row filled (§3.3 with TR5),
    /// the selection, and the buttons' ghosting.
    pub fn refresh(&mut self, ui: &mut Ui, model: &FriendsModel, src: &mut dyn TextSource) {
        let Some(list) = self.list else { return };
        let ids = self.vocab.ids;
        let st = self.vocab.states;
        let rows = model.rows();
        self.rows = ensure_rows(
            ui,
            list,
            self.vocab.layouts.friends,
            ids.friend_template,
            rows.len(),
        );
        self.names = rows.iter().map(|r| r.name.clone()).collect();
        let pids = self.vocab.pids;
        let mut selected_row = None;
        for (k, r) in self.rows.clone().into_iter().zip(&rows) {
            let name = find(ui, k, ids.friend_row_name);
            if let Some(n) = name {
                ui.set_text(n, Some(r.name.clone()));
                let pid = if r.online {
                    pids.online_colour
                } else {
                    pids.offline_colour
                };
                if let Some(argb) = ui.prop(n, pid).value().and_then(as_argb) {
                    ui.set_property(
                        n,
                        pids.text_font_color,
                        PropertyValue::Colour(argb.to_le_bytes()),
                    );
                }
            }
            set_ref(ui, find(ui, k, ids.friend_row_class), Some(&r.class), src);
            set_ref(ui, find(ui, k, ids.friend_row_level), Some(&r.level), src);
            set_ref(ui, find(ui, k, ids.friend_row_area), Some(&r.area), src);
            set_ref(
                ui,
                find(ui, k, ids.friend_row_last_on),
                Some(&r.last_on),
                src,
            );
            if let Some(n) = find(ui, k, ids.friend_row_note) {
                // TR5, TR6: no note; cleared, never the layout's own text.
                ui.set_text(n, Some(String::new()));
            }
            if let Some(g) = find(ui, k, ids.friend_row_grouped) {
                let s = if r.grouped {
                    st.grouped_yes
                } else {
                    st.grouped_unknown
                };
                ui.switch_state_if_defined(g, s);
            }
            if model.selected() == Some(r.name.as_str()) {
                selected_row = Some(k);
            }
        }
        if ui.list_selected(list) != selected_row {
            ui.list_select(list, selected_row, false);
        }
        self.update_buttons(ui, model);
    }

    fn update_buttons(&self, ui: &mut Ui, model: &FriendsModel) {
        let b = model.buttons();
        ghost(ui, self.add, !b.add);
        ghost(ui, self.remove, !b.remove);
        ghost(ui, self.note, !b.note);
        ghost(ui, self.tell, !b.tell);
        ghost(ui, self.invite, !b.invite);
    }

    /// Handles a UI event (§3.4, §3.5): header clicks re-sort (and refresh), a selection
    /// change updates the model and the buttons, buttons and double clicks yield their
    /// actions, a right click on a row selects it and asks for the row menu.
    pub fn handle_event(
        &mut self,
        ui: &mut Ui,
        ev: &UiEvent,
        model: &mut FriendsModel,
        group: GroupState,
        src: &mut dyn TextSource,
    ) -> Vec<SocialAction> {
        let Some(m) = game_message(ev).cloned() else {
            return Vec::new();
        };
        let ms = ui.vocab().msgs;
        let mut out = Vec::new();
        if m.msg == ms.button_click {
            if let Some((_, c)) = self.headers.iter().find(|(k, _)| *k == m.source) {
                model.click_header(*c);
                self.refresh(ui, model, src);
            } else if Some(m.source) == self.add {
                out.push(SocialAction::Dialog(FriendsModel::add_dialog()));
            } else if Some(m.source) == self.remove {
                out.extend(model.remove().map(SocialAction::Intent));
            } else if Some(m.source) == self.tell {
                out.extend(model.tell().map(SocialAction::Intent));
            } else if Some(m.source) == self.invite {
                out.extend(model.invite().map(SocialAction::Intent));
            }
        } else if m.msg == ms.listbox_new_selection && Some(m.source) == self.list {
            let sel = self
                .list
                .and_then(|l| ui.list_selected(l))
                .and_then(|k| self.rows.iter().position(|r| *r == k))
                .and_then(|i| self.names.get(i).cloned());
            model.select(sel.as_deref());
            self.update_buttons(ui, model);
        } else if m.msg == ms.mouse_dbl_click {
            if let Some(i) = row_of(ui, &self.rows, m.source) {
                self.select_row(ui, model, i);
                out.extend(model.tell().map(SocialAction::Intent));
            }
        } else if m.msg == ms.mouse_click
            && mouse_button(&m) == 2
            && let Some(i) = row_of(ui, &self.rows, m.source)
        {
            self.select_row(ui, model, i);
            if let Some(name) = self.names.get(i)
                && let Some(menu) = model.menu(name, group)
            {
                out.push(SocialAction::Menu(menu));
            }
        }
        out
    }

    fn select_row(&mut self, ui: &mut Ui, model: &mut FriendsModel, i: usize) {
        model.select(self.names.get(i).map(String::as_str));
        if let (Some(list), Some(k)) = (self.list, self.rows.get(i).copied())
            && ui.list_selected(list) != Some(k)
        {
            ui.list_select(list, Some(k), false);
        }
        self.update_buttons(ui, model);
    }
}
