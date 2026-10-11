// SPDX-License-Identifier: GPL-3.0-or-later
//! The instant-message tab and its sessions (`chat-window.md` §4.6, §15.4, Tessera rule
//! §15.10 CT6).
//!
//! [`ImSessions`] is the session model: the generic row, the partners' rows in name order,
//! the shown session, unread marks and each session's lines. [`ImTab`] binds it to the
//! elements of the `chatwindow` layout: the tab created from `UI_ChatWindow_IMTabElement`,
//! its log `IMLogElement` (named by the tab's `UI_ChatTab_IMLogElement`), the session list
//! (`UI_ChatLog_IMListboxElement`), the parent of the shown session's log
//! (`UI_ChatLog_IMLogParentField`) and one row from the session list's
//! `UI_ChatLog_IMTextElement` per session with its own log (`UI_ChatTab_LogElement`). The binding lives on
//! [`super::window::Chat`] ([`super::window::Chat::open_im_tab`],
//! [`super::window::Chat::deliver_im`]).

use super::notice::{self as n, Notice, TextRef, is_valid_name, name_equal, tables as T};
use super::social::ImLine;
use crate::tree::ElementKey;

/// Rows the session list holds when its list box gives no `UICore_ListBox_max_rows`
/// (Tessera cap; the data gives 128).
pub const FALLBACK_MAX_ROWS: usize = 128;

/// Lines a session keeps in the model when its log gives no cap (Tessera cap; the data
/// gives 128).
pub const FALLBACK_MAX_LINES: usize = 128;

/// Property IDs of the instant-message elements (property-dictionary names, resolved by
/// label at run time; the defaults are Tessera placeholders no data uses).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImPids {
    /// `UI_ChatTab_IMLogElement` (on the tab template).
    pub tab_im_log: u32,
    /// `UI_ChatLog_IMListboxElement` (on `IMLogElement`).
    pub log_im_listbox: u32,
    /// `UI_ChatLog_IMTextElement` (on the session list: the row template).
    pub log_im_text: u32,
    /// `UI_ChatLog_IMLogParentField` (on `IMLogElement`).
    pub log_im_parent: u32,
    /// `UI_ChatTab_LogElement` (on the row template: the session's log).
    pub tab_log: u32,
}

impl Default for ImPids {
    fn default() -> Self {
        Self {
            tab_im_log: 0xFFFD_4101,
            log_im_listbox: 0xFFFD_4102,
            log_im_text: 0xFFFD_4103,
            log_im_parent: 0xFFFD_4104,
            tab_log: 0xFFFD_4105,
        }
    }
}

impl ImPids {
    /// Every `(label, value)` pair.
    #[must_use]
    pub fn entries(&self) -> Vec<(&'static str, u32)> {
        vec![
            ("UI_ChatTab_IMLogElement", self.tab_im_log),
            ("UI_ChatLog_IMListboxElement", self.log_im_listbox),
            ("UI_ChatLog_IMTextElement", self.log_im_text),
            ("UI_ChatLog_IMLogParentField", self.log_im_parent),
            ("UI_ChatTab_LogElement", self.tab_log),
        ]
    }

    /// Replaces each value whose label `lookup` resolves (the property dictionary).
    pub fn resolve_with(&mut self, lookup: &mut dyn FnMut(&str) -> Option<u32>) {
        let mut set = |label: &str, v: &mut u32| {
            if let Some(x) = lookup(label) {
                *v = x;
            }
        };
        set("UI_ChatTab_IMLogElement", &mut self.tab_im_log);
        set("UI_ChatLog_IMListboxElement", &mut self.log_im_listbox);
        set("UI_ChatLog_IMTextElement", &mut self.log_im_text);
        set("UI_ChatLog_IMLogParentField", &mut self.log_im_parent);
        set("UI_ChatTab_LogElement", &mut self.tab_log);
    }
}

/// One line of a session (§15.4 step 5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImEntry {
    /// The time text (argument 0x58E15).
    pub time: String,
    /// The speaker.
    pub speaker: String,
    /// The text.
    pub text: String,
}

impl ImEntry {
    /// The line *Chat*(0x0F2CF919) with mask 0x20 (§15.4 step 5).
    #[must_use]
    pub fn notice(&self) -> Notice {
        Notice::of(
            TextRef::new(T::CHAT, 0x0F2C_F919)
                .text(n::args::TIME, self.time.clone())
                .text(n::args::NAME, self.speaker.clone())
                .text(n::args::TEXT, self.text.clone()),
            n::TELL,
        )
    }
}

/// One session row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImRow {
    /// The partner's name; `None` for the generic row.
    pub name: Option<String>,
    /// The unread mark (CT6: a row that got a line while not shown).
    pub unread: bool,
    /// The session's lines, oldest first.
    pub lines: Vec<ImEntry>,
}

/// Where a tell line went (§15.4 steps 1–3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImDelivery {
    /// Appended to the row at this index (0 is the generic row).
    Row {
        /// The row index.
        index: usize,
        /// Whether the row was created for it.
        created: bool,
    },
    /// Not shown in this tab (the list is full and auto-creation is on, CW49).
    NotShown,
}

/// The session model (§15.4 with CT6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImSessions {
    rows: Vec<ImRow>,
    shown: usize,
    /// The session list's `max_rows` (the generic row included).
    pub max_rows: usize,
    /// Lines a session keeps.
    pub max_lines: usize,
    /// `GOP_AutoCreateIMSessions`.
    pub auto_create: bool,
}

/// UTF-16 ordinal order (§15.4 step 3: code units, case-sensitive).
fn utf16_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

impl ImSessions {
    /// The model at start (§15.4 "Start"): the generic row, shown.
    #[must_use]
    pub fn new(max_rows: usize, max_lines: usize, auto_create: bool) -> Self {
        Self {
            rows: vec![ImRow {
                name: None,
                unread: false,
                lines: Vec::new(),
            }],
            shown: 0,
            max_rows: max_rows.max(1),
            max_lines: max_lines.max(1),
            auto_create,
        }
    }

    /// The rows; row 0 is the generic row.
    #[must_use]
    pub fn rows(&self) -> &[ImRow] {
        &self.rows
    }

    /// The shown row's index.
    #[must_use]
    pub fn shown(&self) -> usize {
        self.shown
    }

    /// The row of a partner (name-equal, CT6).
    #[must_use]
    pub fn find(&self, partner: &str) -> Option<usize> {
        self.rows
            .iter()
            .position(|r| r.name.as_deref().is_some_and(|x| name_equal(x, partner)))
    }

    /// Inserts a partner's row in name order (after the generic row); `None` when full.
    fn create(&mut self, partner: &str) -> Option<usize> {
        if self.rows.len() >= self.max_rows {
            return None;
        }
        let at = self
            .rows
            .iter()
            .enumerate()
            .skip(1)
            .find(|(_, r)| {
                r.name
                    .as_deref()
                    .is_some_and(|x| utf16_cmp(x, partner) == std::cmp::Ordering::Greater)
            })
            .map_or(self.rows.len(), |(i, _)| i);
        self.rows.insert(
            at,
            ImRow {
                name: Some(partner.to_string()),
                unread: false,
                lines: Vec::new(),
            },
        );
        if self.shown >= at {
            self.shown += 1;
        }
        Some(at)
    }

    /// Renames row `i` to `name` (§15.4 step 4) and moves it to its place in name order,
    /// so that rows stay ordered (CW-I26; Tessera: the original renames in place). Returns
    /// the new index.
    fn rename(&mut self, i: usize, name: &str) -> usize {
        let mut row = self.rows.remove(i);
        let was_shown = self.shown == i;
        if self.shown > i {
            self.shown -= 1;
        }
        row.name = Some(name.to_string());
        let at = self
            .rows
            .iter()
            .enumerate()
            .skip(1)
            .find(|(_, r)| {
                r.name
                    .as_deref()
                    .is_some_and(|x| utf16_cmp(x, name) == std::cmp::Ordering::Greater)
            })
            .map_or(self.rows.len(), |(j, _)| j);
        self.rows.insert(at, row);
        if was_shown {
            self.shown = at;
        } else if self.shown >= at {
            self.shown += 1;
        }
        at
    }

    /// A CT1 `Tell` line's instant-message line (§15.4 steps 1–5 with CT6). `time` is the
    /// local time text (§5.4's Tessera rule). Returns where it went and, when the shown
    /// row was renamed, its new name for the `Tell` mode (step 4).
    pub fn deliver(&mut self, line: &ImLine, time: &str) -> (ImDelivery, Option<String>) {
        let (index, created) = match self.find(&line.partner) {
            Some(i) => (i, false),
            None if self.auto_create => match self.create(&line.partner) {
                Some(i) => (i, true),
                None => return (ImDelivery::NotShown, None),
            },
            None => (0, false),
        };
        let mut index = index;
        let mut renamed = None;
        if index != 0 && self.rows[index].name.as_deref() != Some(line.partner.as_str()) {
            index = self.rename(index, &line.partner);
            if self.shown == index {
                renamed = Some(line.partner.clone());
            }
        }
        let max = self.max_lines;
        let row = &mut self.rows[index];
        row.lines.push(ImEntry {
            time: time.to_string(),
            speaker: line.speaker.clone(),
            text: line.text.clone(),
        });
        if row.lines.len() > max {
            let extra = row.lines.len() - max;
            row.lines.drain(..extra);
        }
        if self.shown != index {
            row.unread = true;
        }
        (ImDelivery::Row { index, created }, renamed)
    }

    /// Shows row `i` (a click on its row): clears its unread mark and returns the `Tell`
    /// target (empty for the generic row), or `None` for an index out of range.
    pub fn show(&mut self, i: usize) -> Option<String> {
        let row = self.rows.get_mut(i)?;
        row.unread = false;
        self.shown = i;
        Some(row.name.clone().unwrap_or_default())
    }

    /// Whether "start session" is enabled (the row count below `max_rows`).
    #[must_use]
    pub fn can_start(&self) -> bool {
        self.rows.len() < self.max_rows
    }

    /// "Start session" with the prompt's answer (CT6): an empty answer posts
    /// *Chat*(0x0EA68129), an invalid name a line in Tessera's words, both with mask 0x1;
    /// an existing partner's row is shown; else a row is created and shown. Returns the
    /// shown row's index.
    ///
    /// # Errors
    /// The refusal line.
    pub fn start(&mut self, answer: &str) -> Result<usize, Notice> {
        let name = answer.trim();
        if name.is_empty() {
            return Err(Notice::of(TextRef::new(T::CHAT, 0x0EA6_8129), n::ERROR));
        }
        if !is_valid_name(name) {
            return Err(Notice::own("That is not a valid character name.", n::ERROR));
        }
        let i = match self.find(name) {
            Some(i) => i,
            None => self
                .create(name)
                .ok_or_else(|| Notice::own("No more conversations can be opened.", n::ERROR))?,
        };
        self.show(i);
        Ok(i)
    }

    /// "Close session" (CT6): removes row `i` (never the generic row); the generic row is
    /// shown when `i` was. Returns whether a row was removed.
    pub fn close(&mut self, i: usize) -> bool {
        if i == 0 || i >= self.rows.len() {
            return false;
        }
        self.rows.remove(i);
        if self.shown == i {
            self.shown = 0;
        } else if self.shown > i {
            self.shown -= 1;
        }
        true
    }
}

/// One session row's elements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImRowElements {
    /// The row (in the session list).
    pub row: ElementKey,
    /// The row's label (its first text element, else the row).
    pub label: ElementKey,
    /// The row's unread marker (`UnreadIndicator`).
    pub unread: Option<ElementKey>,
    /// The session's log (in the parent field).
    pub log: ElementKey,
    /// The log's list box.
    pub list: Option<ElementKey>,
    /// The log's line template (`UI_ChatLog_TextElement`).
    pub line_template: u32,
}

/// The instant-message tab on the element tree.
#[derive(Debug, Clone, PartialEq)]
pub struct ImTab {
    /// The window (index into [`super::window::Chat::windows`]).
    pub window: usize,
    /// The tab element (in the tab list).
    pub tab: ElementKey,
    /// The tab's unread marker.
    pub unread: Option<ElementKey>,
    /// `IMLogElement`.
    pub log: ElementKey,
    /// The session list.
    pub sessions_list: ElementKey,
    /// The parent of the shown session's log.
    pub parent: ElementKey,
    /// The row template's element ID.
    pub row_template: u32,
    /// The generic row's label (*Chat*(0x0D93B0C5), as rendered by the client).
    pub generic_label: String,
    /// The row elements, in model order: always as many as the model has rows, the
    /// generic row first.
    pub rows: Vec<ImRowElements>,
    /// The model.
    pub model: ImSessions,
    /// Whether the tab is the selected tab of its window.
    pub selected: bool,
}

// ----- binding on the chat windows ------------------------------------------------------

use tessera_formats::property::PropertyValue;

use super::notice::TextSource;
use super::window::Chat;
use crate::game::{GameError, find_below, subtree};
use crate::ui::Ui;
use crate::widgets::WidgetClass;

/// Sets a list box's entry order (entries already in it) and lays it out.
fn set_list_order(ui: &mut Ui, list: ElementKey, order: Vec<ElementKey>) {
    if let Some(l) = ui.el_mut(list).and_then(|e| e.widget.list.as_mut()) {
        if l.entries == order {
            return;
        }
        l.entries = order;
        l.dirty = true;
    }
    ui.list_layout_if_dirty(list);
}

/// The widget class of an element.
fn class(ui: &Ui, k: ElementKey) -> Option<WidgetClass> {
    ui.el(k).map(|e| e.class)
}

/// A positive integer property, else `fallback`.
fn cap(ui: &Ui, k: ElementKey, fallback: usize) -> usize {
    let v = ui.prop(k, ui.vocab().pids.lb_max_rows).int();
    usize::try_from(v)
        .ok()
        .filter(|c| *c > 0)
        .unwrap_or(fallback)
        .min(super::window::FALLBACK_LINE_CAP)
}

impl Chat {
    /// Creates the instant-message tab in the primary window (§4.6, CT6), when
    /// `GOP_FullIRCMode` is true (the caller's test) and none exists: the tab from
    /// `UI_ChatWindow_IMTabElement` labelled `label` (*Chat*(0x0E3E83D5), rendered by the
    /// client), marked `UI_ChatWindow_IsIMTab` and not draggable, its log in the window's
    /// log field (hidden until the tab is selected), and the generic row labelled
    /// `generic_label` (*Chat*(0x0D93B0C5)), shown. `auto_create` is
    /// `GOP_AutoCreateIMSessions`.
    ///
    /// # Errors
    /// [`GameError::NoSuchWindow`] without a primary window, [`GameError::MissingProperty`]
    /// or [`GameError::MissingElement`] for a missing part (also when the generic row's
    /// elements cannot be created: [`GameError::MissingElement`] of the row template), or a
    /// creation error; nothing is left behind.
    pub fn open_im_tab(
        &mut self,
        ui: &mut Ui,
        label: &str,
        generic_label: &str,
        auto_create: bool,
    ) -> Result<(), GameError> {
        if self.im.is_some() {
            return Ok(());
        }
        let p = self.primary().ok_or(GameError::NoSuchWindow(0))?;
        let gp = self.vocab.pids;
        let ip = self.im_pids;
        let ids = self.vocab.ids;
        let win = self.windows[p].clone();
        let template = ui.prop(win.root, gp.chat_im_tab_element).key();
        if template == 0 {
            return Err(GameError::MissingProperty(gp.chat_im_tab_element));
        }
        let tab_list = win
            .tab_list
            .ok_or(GameError::MissingElement(ids.chat_tab_list))?;
        let log_parent = win
            .log_parent
            .ok_or(GameError::MissingProperty(gp.chat_log_parent))?;
        let tab = ui.create_element(None, self.layout, template)?;
        let built = (|| -> Result<(ElementKey, ElementKey, ElementKey, u32), GameError> {
            let log_id = ui.prop(tab, ip.tab_im_log).key();
            if log_id == 0 {
                return Err(GameError::MissingProperty(ip.tab_im_log));
            }
            let log = ui.create_element(Some(log_parent), self.layout, log_id)?;
            let found = (|| {
                let list_id = ui.prop(log, ip.log_im_listbox).key();
                let parent_id = ui.prop(log, ip.log_im_parent).key();
                let list =
                    find_below(ui, log, list_id).ok_or(GameError::MissingElement(list_id))?;
                // The session list names its row template (§15.4); the log is the fallback.
                let row_template = match ui.prop(list, ip.log_im_text).key() {
                    0 => ui.prop(log, ip.log_im_text).key(),
                    t => t,
                };
                let parent =
                    find_below(ui, log, parent_id).ok_or(GameError::MissingElement(parent_id))?;
                if row_template == 0 {
                    return Err(GameError::MissingProperty(ip.log_im_text));
                }
                Ok((list, parent, row_template))
            })();
            match found {
                Ok((list, parent, row_template)) => Ok((log, list, parent, row_template)),
                Err(e) => {
                    ui.destroy(log);
                    Err(e)
                }
            }
        })();
        let (log, sessions_list, parent, row_template) = match built {
            Ok(b) => b,
            Err(e) => {
                ui.destroy(tab);
                return Err(e);
            }
        };
        if let Err(e) = ui.list_add(tab_list, tab, None) {
            ui.destroy(log);
            ui.destroy(tab);
            return Err(e.into());
        }
        ui.set_text(tab, Some(label.to_string()));
        ui.set_property(tab, gp.chat_is_im_tab, PropertyValue::Bool(true));
        let dragable = ui.vocab().pids.dragable;
        ui.set_property(tab, dragable, PropertyValue::Bool(false));
        let unread = find_below(ui, tab, ids.unread_indicator);
        if let Some(u) = unread {
            ui.set_shown(u, false);
        }
        ui.set_shown(log, false);
        if let Some(id) = ui.element_id(sessions_list) {
            ui.register_handler(id, ui.vocab().msgs.listbox_new_selection);
        }
        let max_rows = cap(ui, sessions_list, FALLBACK_MAX_ROWS);
        let mut im = ImTab {
            window: p,
            tab,
            unread,
            log,
            sessions_list,
            parent,
            row_template,
            generic_label: generic_label.to_string(),
            rows: Vec::new(),
            model: ImSessions::new(max_rows, FALLBACK_MAX_LINES, auto_create),
            selected: false,
        };
        // The generic row is mandatory: the element rows must stay aligned with the
        // model's rows, whose first is the generic row.
        match self.im_row_elements(ui, &im, generic_label) {
            Some(r) => im.rows.push(r),
            None => {
                tracing::debug!("instant-message generic row not created");
                ui.destroy(log);
                ui.list_remove(tab_list, tab);
                ui.destroy(tab);
                return Err(GameError::MissingElement(row_template));
            }
        }
        if let Some(r) = im.rows.first() {
            im.model.max_lines = cap(ui, r.list.unwrap_or(r.log), FALLBACK_MAX_LINES);
        }
        self.im = Some(im);
        self.im_sync(ui);
        Ok(())
    }

    /// Creates one session row and its log (hidden), labelled `label`.
    fn im_row_elements(&self, ui: &mut Ui, im: &ImTab, label: &str) -> Option<ImRowElements> {
        let ids = self.vocab.ids;
        let row = ui.create_element(None, self.layout, im.row_template).ok()?;
        if ui.list_add(im.sessions_list, row, None).is_err() {
            ui.destroy(row);
            return None;
        }
        let log_id = ui.prop(row, self.im_pids.tab_log).key();
        let log = match ui.create_element(Some(im.parent), self.layout, log_id) {
            Ok(l) => l,
            Err(e) => {
                tracing::debug!("instant-message session log not created: {e}");
                ui.list_remove(im.sessions_list, row);
                return None;
            }
        };
        ui.set_shown(log, false);
        let list = find_below(ui, log, ids.chat_log_listbox).or_else(|| {
            subtree(ui, log)
                .into_iter()
                .skip(1)
                .find(|k| class(ui, *k) == Some(WidgetClass::ListBox))
        });
        let line_template = ui.prop(log, self.vocab.pids.chat_log_text_element).key();
        let label_el = subtree(ui, row)
            .into_iter()
            .skip(1)
            .find(|k| class(ui, *k) == Some(WidgetClass::Text))
            .unwrap_or(row);
        ui.set_text(label_el, Some(label.to_string()));
        let unread = find_below(ui, row, ids.unread_indicator);
        if let Some(u) = unread {
            ui.set_shown(u, false);
        }
        Some(ImRowElements {
            row,
            label: label_el,
            unread,
            log,
            list,
            line_template,
        })
    }

    /// Brings the row elements in line with the model: order in the session list, labels,
    /// unread markers, the shown session's log.
    fn im_sync(&mut self, ui: &mut Ui) {
        let Some(im) = self.im.as_ref() else { return };
        let order: Vec<ElementKey> = im.rows.iter().map(|r| r.row).collect();
        set_list_order(ui, im.sessions_list, order);
        let shown = im.model.shown();
        for (i, (r, m)) in im.rows.iter().zip(im.model.rows()).enumerate() {
            if let Some(name) = &m.name {
                ui.set_text(r.label, Some(name.clone()));
            }
            if let Some(u) = r.unread {
                ui.set_shown(u, m.unread);
            }
            ui.set_shown(r.log, i == shown);
        }
        if let Some(r) = im.rows.get(shown)
            && ui.list_selected(im.sessions_list) != Some(r.row)
        {
            ui.list_select(im.sessions_list, Some(r.row), false);
        }
    }

    /// Rebuilds the element rows after a model change that may have created, moved or
    /// removed rows: elements are matched to model rows by name (name-equal; the generic
    /// row stays first); a model row without elements gets new ones, elements without a
    /// model row are destroyed.
    fn im_rematch(&mut self, ui: &mut Ui, old_names: &[Option<String>]) {
        let Some(mut im) = self.im.take() else { return };
        let mut pool: Vec<(Option<String>, ImRowElements)> =
            old_names.iter().cloned().zip(im.rows.drain(..)).collect();
        let mut rows = Vec::new();
        let mut missing = Vec::new();
        for (i, m) in im.model.rows().iter().enumerate() {
            let pos = pool.iter().position(|(n, _)| match (n, &m.name) {
                (None, None) => true,
                (Some(a), Some(b)) => name_equal(a, b),
                _ => false,
            });
            match pos {
                Some(p) => rows.push(Some(pool.remove(p).1)),
                None => {
                    rows.push(None);
                    missing.push(i);
                }
            }
        }
        for (_, r) in pool {
            ui.destroy(r.log);
            ui.list_remove(im.sessions_list, r.row);
        }
        for i in missing {
            let label = im.model.rows()[i]
                .name
                .clone()
                .unwrap_or_else(|| im.generic_label.clone());
            rows[i] = self.im_row_elements(ui, &im, &label);
        }
        if rows.first().is_some_and(Option::is_none) {
            // The generic row cannot be dropped from the model, so without its elements
            // the rows cannot stay aligned: the tab goes (it is created again on demand).
            tracing::debug!("instant-message generic row lost: the tab is closed");
            for r in rows.into_iter().flatten() {
                ui.destroy(r.log);
            }
            ui.destroy(im.log);
            let tab_list = self.windows.get(im.window).and_then(|w| w.tab_list);
            match tab_list {
                Some(tl) => ui.list_remove(tl, im.tab),
                None => ui.destroy(im.tab),
            }
            if im.selected
                && let Some(sel) = self.windows.get(im.window).map(|w| w.selected)
            {
                self.select_tab(ui, im.window, sel);
            }
            return;
        }
        // A row whose elements could not be created leaves the model too.
        let mut i = 0;
        while i < rows.len() {
            if rows[i].is_none() && i > 0 {
                rows.remove(i);
                im.model.close(i);
            } else {
                i += 1;
            }
        }
        im.rows = rows.into_iter().flatten().collect();
        self.im = Some(im);
        self.im_sync(ui);
    }

    /// Selects the instant-message tab (a click on it in the tab list): its log shown, the
    /// window's other logs hidden, its unread marker hidden.
    pub fn select_im_tab(&mut self, ui: &mut Ui) {
        let Some(im) = self.im.as_mut() else { return };
        im.selected = true;
        let (w, log, tab, unread) = (im.window, im.log, im.tab, im.unread);
        if let Some(win) = self.windows.get(w) {
            for t in &win.tabs {
                ui.set_shown(t.log, false);
            }
            if let Some(tl) = win.tab_list
                && ui.list_selected(tl) != Some(tab)
            {
                ui.list_select(tl, Some(tab), false);
            }
        }
        ui.set_shown(log, true);
        if let Some(u) = unread {
            ui.set_shown(u, false);
        }
    }

    /// Hides the instant-message log when another tab of window `w` is selected.
    pub(crate) fn hide_im_log(&mut self, ui: &mut Ui, w: usize) {
        if let Some(im) = self.im.as_mut()
            && im.window == w
        {
            im.selected = false;
            ui.set_shown(im.log, false);
        }
    }

    /// A CT1 `Tell` line's instant-message line (CT6): appended only to its session's log
    /// (the normal tabs got the CT1 line). `time` is the local time text; the line is
    /// rendered through `src` (*Chat*(0x0F2CF919), mask 0x20). Returns where it went;
    /// `None` without an instant-message tab.
    pub fn deliver_im(
        &mut self,
        ui: &mut Ui,
        line: &ImLine,
        time: &str,
        src: &mut dyn TextSource,
    ) -> Option<ImDelivery> {
        let im = self.im.as_mut()?;
        let old: Vec<Option<String>> = im.model.rows().iter().map(|r| r.name.clone()).collect();
        let (d, renamed) = im.model.deliver(line, time);
        let ImDelivery::Row { index, .. } = d else {
            return Some(d);
        };
        let entry = im.model.rows()[index].lines.last().cloned();
        let name = im.model.rows()[index].name.clone();
        self.im_rematch(ui, &old);
        let im = self.im.as_ref()?;
        // Rows whose elements could not be created left the model: find the row again.
        let at = match &name {
            None => Some(0),
            Some(n) => im.model.find(n),
        };
        let Some(r) = at.and_then(|i| im.rows.get(i)).cloned() else {
            return Some(ImDelivery::NotShown);
        };
        let tab_selected = im.selected;
        let tab_unread = im.unread;
        if let (Some(e), Some(list)) = (entry, r.list) {
            let text = e.notice().render(src, &|c| c.to_string());
            self.append_to(ui, list, r.line_template, super::notice::TELL, &text);
        }
        if !tab_selected && let Some(u) = tab_unread {
            ui.set_shown(u, true);
        }
        if let Some(t) = renamed {
            self.set_tell_mode(ui, &t);
        }
        Some(d)
    }

    /// The session list's selection changed (a click on a row): the row is shown and the
    /// entry's mode becomes `Tell` with the row's name (empty for the generic row).
    pub(crate) fn im_list_selection(&mut self, ui: &mut Ui) {
        let Some(im) = self.im.as_mut() else { return };
        let Some(sel) = ui.list_selected(im.sessions_list) else {
            return;
        };
        let Some(i) = im.rows.iter().position(|r| r.row == sel) else {
            return;
        };
        let Some(target) = im.model.show(i) else {
            return;
        };
        self.im_sync(ui);
        self.set_tell_mode(ui, &target);
    }

    /// Shows session row `i` as a click on it does. Returns the `Tell` target.
    pub fn im_show(&mut self, ui: &mut Ui, i: usize) -> Option<String> {
        let target = self.im.as_mut()?.model.show(i)?;
        self.im_sync(ui);
        self.set_tell_mode(ui, &target);
        Some(target)
    }

    /// "Start session" with the prompt's answer (CT6).
    ///
    /// # Errors
    /// The refusal line to post (mask 0x1), also without an instant-message tab.
    pub fn im_start(&mut self, ui: &mut Ui, answer: &str) -> Result<usize, Notice> {
        let Some(im) = self.im.as_mut() else {
            return Err(Notice::own("Conversations are not available.", n::ERROR));
        };
        let old: Vec<Option<String>> = im.model.rows().iter().map(|r| r.name.clone()).collect();
        let i = im.model.start(answer)?;
        self.im_rematch(ui, &old);
        let target = self
            .im
            .as_ref()
            .and_then(|im| im.model.rows().get(i))
            .and_then(|r| r.name.clone())
            .unwrap_or_default();
        self.set_tell_mode(ui, &target);
        Ok(i)
    }

    /// "Close session" on row `i` (CT6): the row and its log go (never the generic row);
    /// the generic row is shown if `i` was. Returns whether a row was removed.
    pub fn im_close(&mut self, ui: &mut Ui, i: usize) -> bool {
        let Some(im) = self.im.as_mut() else {
            return false;
        };
        let old: Vec<Option<String>> = im.model.rows().iter().map(|r| r.name.clone()).collect();
        if !im.model.close(i) {
            return false;
        }
        self.im_rematch(ui, &old);
        true
    }
}
