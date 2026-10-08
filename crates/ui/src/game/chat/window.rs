// SPDX-License-Identifier: GPL-3.0-or-later
//! The chat windows on the element tree (`chat-window.md` §3–§6, §10): windows from the
//! element pool, tabs and their logs, appending lines, the unread marker, the entry line's
//! commands, the channel-mode switch, opacity, and paging.

use std::collections::BTreeMap;

use tessera_formats::did::Did;
use tessera_formats::property::{PropertyArray, PropertyEntry, PropertyValue};
use tessera_formats::string_info::StringInfo;

use crate::events::{Message, UiEvent};
use crate::props::as_int;
use crate::tree::ElementKey;
use crate::ui::Ui;
use crate::vocab::input_maps;

use super::super::vocab::GameVocab;
use super::super::{GameError, find_below, set_image, subtree};
use super::command::{CommandHost, CommandLine, RESULT_UNKNOWN};
use super::config::{ChatConfig, TabConfig};
use super::entry::{
    ChannelModes, GuardVerdict, KeyRepeatGuard, SAY, TELL, at_bottom, page_targets,
};
use super::mask::{FeedbackSounds, OpacityTable, StyleMap, UNFILTERED, log_takes};

/// The run-time text of a line (or any element), without resolving its layout text.
#[must_use]
pub fn line_text(ui: &Ui, k: ElementKey) -> String {
    ui.el(k)
        .and_then(|e| e.text.runtime.clone())
        .unwrap_or_default()
}

/// Lines a log keeps when its list box has no usable `UICore_ListBox_max_rows` (Tessera
/// cap; the data gives 128, CW-I8).
pub const FALLBACK_LINE_CAP: usize = 4096;

/// Draw mode of the opacity image (§3.5 step 2: mode value 4; its meaning is Q2).
pub const OPACITY_DRAW_MODE: u32 = 4;

/// Mask of the reply line (§6.2: types 1 and 6).
pub const REPLY_MASK: u64 = 0x21;

/// Mask of the unknown-command line (§7.6: type 1).
pub const ERROR_MASK: u64 = 0x1;

/// Game text the chat window shows, resolved from the user's install by the client
/// (string tables named in `chat-window.md`), or Tessera's own wording.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChatTexts {
    /// `CommandSetup` token 0x04FE7F13 (§7.6).
    pub unknown_command: String,
    /// `Chat` token 0x00CC89CC (§6.2 `ChatModeReply` without a target).
    pub no_reply_target: String,
    /// Display names of channels by chat type, for the `OutputTextType` label (§6.5).
    pub channel_names: BTreeMap<u32, String>,
}

/// One tab and its log.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatTab {
    /// The tab element (in the tab list).
    pub tab: ElementKey,
    /// The `ChatLog` element.
    pub log: ElementKey,
    /// Its `ChatLogListbox`.
    pub list: Option<ElementKey>,
    /// `UI_ChatLog_TextElement` (the line template's element ID).
    pub line_template: u32,
    /// `UnreadIndicator`.
    pub unread: Option<ElementKey>,
    /// The tab's label as configured.
    pub name: Option<StringInfo>,
    /// The type mask.
    pub mask: u64,
    /// The channel mode stored on the tab (§6.5).
    pub mode: u32,
}

/// One chat window.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatWindow {
    /// Window ID (§3.2).
    pub id: i32,
    /// The root element.
    pub root: ElementKey,
    /// `UI_ChatWindow_PrimaryWindow`.
    pub primary: bool,
    /// The tab list (`ChatWindowTabListBox`).
    pub tab_list: Option<ElementKey>,
    /// `UI_ChatWindow_LogParentField`.
    pub log_parent: Option<ElementKey>,
    /// `UI_ChatWindow_TextEntryElement` (primary only).
    pub entry: Option<ElementKey>,
    /// `UI_ChatWindow_OutputTextElement`.
    pub output_label: Option<ElementKey>,
    /// `UI_ChatWindow_TabElement`.
    pub tab_template: u32,
    /// Tabs in tab-list order.
    pub tabs: Vec<ChatTab>,
    /// The selected tab.
    pub selected: usize,
}

/// The chat windows of the client.
#[derive(Debug, Clone)]
pub struct Chat {
    /// Labels.
    pub vocab: GameVocab,
    /// The `chatwindow` layout.
    pub layout: Did,
    /// Windows in window-ID order.
    pub windows: Vec<ChatWindow>,
    /// Line styles and fonts (`fonts.md` §9.3).
    pub styles: StyleMap,
    /// Feedback sounds (§5.6).
    pub sounds: FeedbackSounds,
    /// Background opacities (§3.5).
    pub opacity: OpacityTable,
    /// The command line and its history.
    pub command_line: CommandLine,
    /// The mode table.
    pub modes: ChannelModes,
    /// The channel mode (a chat type).
    pub mode: u32,
    /// The previous mode.
    pub previous_mode: u32,
    /// The tell target.
    pub target: String,
    /// The key-repeat guard, present while `GOP_ChatKeyRepeatProtection` is on.
    pub guard: Option<KeyRepeatGuard>,
    /// Game text.
    pub texts: ChatTexts,
    /// The result of the last submitted line.
    pub last_result: Option<i32>,
    pool: Vec<(i32, u32)>,
}

/// What the chat window did with a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandOutcome {
    /// Not handled: the command goes on to game code (decision 0014 D7).
    NotHandled,
    /// Handled.
    Handled,
    /// A line was submitted with this result (§7.4).
    Submitted(i32),
}

/// What [`Chat::text_input`] did with a typed character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextInputOutcome {
    /// No element took it.
    NotTaken,
    /// It was inserted (or otherwise taken by the focused element).
    Taken,
    /// The key-repeat guard swallowed it (§6.6 step 3): the entry lost focus and the
    /// repeated characters were deleted. The client plays UI sound
    /// [`super::entry::GUARD_SOUND`] once for this outcome.
    GuardTripped,
}

impl Chat {
    /// A chat system for the `chatwindow` layout, with no windows yet.
    #[must_use]
    pub fn new(vocab: GameVocab, layout: Did) -> Self {
        Self {
            vocab,
            layout,
            windows: Vec::new(),
            styles: StyleMap::default(),
            sounds: FeedbackSounds::default(),
            opacity: OpacityTable::default(),
            command_line: CommandLine::default(),
            modes: ChannelModes::default(),
            mode: SAY,
            previous_mode: SAY,
            target: String::new(),
            guard: None,
            texts: ChatTexts::default(),
            last_result: None,
            pool: Vec::new(),
        }
    }

    /// The pool of window elements (§3.2 step 1): (window ID, element ID) of every
    /// top-level element of the layout that carries `UI_ChatWindow_WindowID`.
    fn fill_pool(&mut self, ui: &mut Ui) {
        self.pool.clear();
        let Some(l) = ui.env.layout(self.layout) else {
            return;
        };
        let pid = self.vocab.pids.chat_window_id;
        for d in &l.elements {
            if let Some(v) = d.block.properties.entries.iter().find(|e| e.pid == pid) {
                self.pool.push((as_int(&v.value), d.element_id));
            }
        }
    }

    /// Creates the windows and tabs of a configuration (§3.2 step 5, §3.3 loading; §10
    /// item 1). Windows whose ID names no free pool element are skipped. A window or tab
    /// that cannot be created is skipped too (a window's pool element goes back to the
    /// pool); the others are still created, and every created window takes its configured
    /// visibility.
    ///
    /// # Errors
    /// The first creation error, after everything else was created.
    pub fn open(&mut self, ui: &mut Ui, config: &ChatConfig) -> Result<(), GameError> {
        if self.pool.is_empty() && self.windows.is_empty() {
            self.fill_pool(ui);
        }
        let mut first_err = None;
        for w in &config.windows {
            let Some(pos) = self.pool.iter().position(|(id, _)| *id == w.id) else {
                tracing::debug!("chat window {} has no free element", w.id);
                continue;
            };
            let (wid, element) = self.pool.remove(pos);
            let root = match ui.open_panel(self.layout, element) {
                Ok(r) => r,
                Err(e) => {
                    tracing::debug!("chat window {} not opened: {e}", w.id);
                    self.pool.insert(pos, (wid, element));
                    first_err.get_or_insert(e.into());
                    continue;
                }
            };
            let win = self.init_window(ui, w.id, root);
            let idx = self.insert_window(win);
            for t in &w.tabs {
                if let Err(e) = self.add_tab(ui, idx, t) {
                    tracing::debug!("chat window {}: tab not created: {e}", w.id);
                    first_err.get_or_insert(e);
                }
            }
            if !self.windows[idx].tabs.is_empty() {
                self.select_tab(ui, idx, 0);
            }
            ui.set_shown(root, w.visible);
        }
        first_err.map_or(Ok(()), Err)
    }

    fn insert_window(&mut self, w: ChatWindow) -> usize {
        let idx = self
            .windows
            .iter()
            .position(|x| x.id > w.id)
            .unwrap_or(self.windows.len());
        self.windows.insert(idx, w);
        idx
    }

    fn init_window(&mut self, ui: &mut Ui, id: i32, root: ElementKey) -> ChatWindow {
        let p = self.vocab.pids;
        let primary = ui.prop(root, p.chat_primary).bool();
        let by_prop = |ui: &Ui, pid: u32| {
            let v = ui.prop(root, pid).key();
            if v == 0 {
                None
            } else {
                find_below(ui, root, v)
            }
        };
        let tab_list = subtree(ui, root)
            .into_iter()
            .find(|k| ui.element_type(*k) == Some(self.vocab.types.chat_tab_list))
            .or_else(|| find_below(ui, root, self.vocab.ids.chat_tab_list));
        let entry = if primary {
            by_prop(ui, p.chat_text_entry_element)
        } else {
            None
        };
        let w = ChatWindow {
            id,
            root,
            primary,
            tab_list,
            log_parent: by_prop(ui, p.chat_log_parent),
            entry,
            output_label: by_prop(ui, p.chat_output_text),
            tab_template: ui.prop(root, p.chat_tab_element).key(),
            tabs: Vec::new(),
            selected: 0,
        };
        if let Some(t) = tab_list
            && let Some(tid) = ui.element_id(t)
        {
            ui.register_handler(tid, ui.vocab().msgs.listbox_new_selection);
        }
        ui.set_element_maps(root, vec![input_maps::CHAT_LOG.to_string()]);
        if primary {
            self.mode = SAY;
            self.update_label(ui, &w);
        }
        w
    }

    /// Adds a tab with its log to window `w` (§3.3 loading). Nothing is left behind when it
    /// fails: a tab whose log cannot be created is removed from the tab list again.
    ///
    /// # Errors
    /// [`GameError::NoSuchWindow`] for an index outside the windows,
    /// [`GameError::MissingElement`] without a tab list or when the log parent is not found,
    /// [`GameError::MissingProperty`] when the window names no log parent, or a creation
    /// error.
    pub fn add_tab(&mut self, ui: &mut Ui, w: usize, cfg: &TabConfig) -> Result<(), GameError> {
        let ids = self.vocab.ids;
        let pid = self.vocab.pids.chat_log_parent;
        let (tab_list, log_parent, template) = {
            let win = self.windows.get(w).ok_or(GameError::NoSuchWindow(w))?;
            let missing_parent = || {
                let v = ui.prop(win.root, pid).key();
                if v == 0 {
                    GameError::MissingProperty(pid)
                } else {
                    GameError::MissingElement(v)
                }
            };
            (
                win.tab_list
                    .ok_or(GameError::MissingElement(ids.chat_tab_list))?,
                win.log_parent.ok_or_else(missing_parent)?,
                win.tab_template,
            )
        };
        let tab = ui.create_element(None, self.layout, template)?;
        if let Err(e) = ui.list_add(tab_list, tab, None) {
            ui.destroy(tab);
            return Err(e.into());
        }
        let label = cfg
            .name
            .as_ref()
            .and_then(|n| ui.env.resolve_string(n))
            .unwrap_or_default();
        ui.set_text(tab, Some(label));
        let log = match ui.create_element(Some(log_parent), self.layout, ids.chat_log_element) {
            Ok(l) => l,
            Err(e) => {
                ui.list_remove(tab_list, tab);
                return Err(e.into());
            }
        };
        let list = find_below(ui, log, ids.chat_log_listbox);
        let line_template = ui.prop(log, self.vocab.pids.chat_log_text_element).key();
        let unread = find_below(ui, tab, ids.unread_indicator);
        if let Some(u) = unread {
            ui.set_shown(u, false);
        }
        ui.set_shown(log, false);
        let win = &mut self.windows[w];
        win.tabs.push(ChatTab {
            tab,
            log,
            list,
            line_template,
            unread,
            name: cfg.name.clone(),
            mask: cfg.mask,
            mode: SAY,
        });
        Ok(())
    }

    /// Selects tab `i` of window `w` (§4.2): highlighted in the tab list, its log shown in
    /// the log field (Tessera rule of Q3: every log is a child of the log field and only
    /// the selected one is shown), its unread marker hidden (§4.4), and — in the primary
    /// window — the channel mode taken from the tab (§6.5).
    pub fn select_tab(&mut self, ui: &mut Ui, w: usize, i: usize) {
        let Some(win) = self.windows.get_mut(w) else {
            return;
        };
        if i >= win.tabs.len() {
            return;
        }
        win.selected = i;
        let win = win.clone();
        if let Some(tl) = win.tab_list
            && ui.list_selected(tl) != Some(win.tabs[i].tab)
        {
            ui.list_select(tl, Some(win.tabs[i].tab), false);
        }
        for (j, t) in win.tabs.iter().enumerate() {
            ui.set_shown(t.log, j == i);
        }
        if let Some(u) = win.tabs[i].unread {
            ui.set_shown(u, false);
        }
        if win.primary {
            self.mode = win.tabs[i].mode;
            self.update_label(ui, &win);
        }
    }

    /// Toggles chat types `m` in tab `i` of window `w` (§4.1, §4.5): the mask is XORed;
    /// lines already in the log stay. The client saves the configuration after it.
    pub fn toggle_filter(&mut self, w: usize, i: usize, m: u64) {
        if let Some(t) = self.windows.get_mut(w).and_then(|w| w.tabs.get_mut(i)) {
            t.mask ^= m;
        }
    }

    /// The current configuration (§3.3 saving).
    #[must_use]
    pub fn config(&self, ui: &Ui) -> ChatConfig {
        ChatConfig {
            windows: self
                .windows
                .iter()
                .map(|w| super::config::WindowConfig {
                    id: w.id,
                    visible: ui.is_visible(w.root),
                    tabs: w
                        .tabs
                        .iter()
                        .map(|t| TabConfig {
                            name: t.name.clone(),
                            mask: t.mask,
                        })
                        .collect(),
                })
                .collect(),
        }
    }

    /// Delivers a line to every log (§4.3; Q4 Tessera rule: windows in window-ID order,
    /// tabs in tab-list order, no suppression flag, no refill). Returns the number of logs
    /// that took it.
    pub fn post_line(&mut self, ui: &mut Ui, mask: u64, text: &str) -> usize {
        let mut n = 0;
        for w in 0..self.windows.len() {
            for i in 0..self.windows[w].tabs.len() {
                if log_takes(self.windows[w].tabs[i].mask, mask) {
                    self.append(ui, w, i, mask, text);
                    n += 1;
                }
            }
        }
        n
    }

    /// Appends a line to one log (§5.1). Timestamps (step 7) are applied by the client
    /// before posting, since Tessera applies no text filter (Q7).
    pub fn append(&mut self, ui: &mut Ui, w: usize, i: usize, mask: u64, text: &str) {
        let Some(tab) = self.windows.get(w).and_then(|w| w.tabs.get(i)).cloned() else {
            return;
        };
        let selected = self.windows[w].selected == i;
        let Some(list) = tab.list else { return };
        // Steps 1–2.
        let t = mask & !UNFILTERED;
        let style = self.styles.style(t);
        // Step 3.
        let entries = ui.list_entries(list);
        let (content, offset) = ui.scroll_state(list).unwrap_or(((0, 0), (0, 0)));
        let v = ui.layout_box(list).map_or(0, |b| b.h());
        let h = if entries.len() > 1 {
            entries
                .last()
                .and_then(|e| ui.layout_box(*e))
                .map_or(0, |b| b.h())
        } else {
            0
        };
        let was_bottom = at_bottom(offset.1, v, content.1, h);
        // Step 4.
        let cap = ui.prop(list, ui.vocab().pids.lb_max_rows).int();
        let cap = usize::try_from(cap)
            .ok()
            .filter(|c| *c > 0)
            .unwrap_or(FALLBACK_LINE_CAP)
            .min(FALLBACK_LINE_CAP);
        let mut entries = entries;
        while entries.len() > cap {
            let first = entries.remove(0);
            ui.list_remove(list, first);
        }
        // Step 4 with step 5 at the cap: the oldest line leaves the log and its element is
        // reused for the new line (Tessera: destroying an element costs a pass over the
        // whole UI, decision 0015 U4), which is the same as removing it and creating a
        // new line from the template.
        let recycled = if entries.len() == cap && !entries.is_empty() {
            let first = entries.remove(0);
            if ui.list_selected(list) == Some(first) {
                ui.list_select(list, None, false);
            }
            ui.detach_raw(first);
            match ui.list_add(list, first, None) {
                Ok(()) => Some(first),
                Err(e) => {
                    tracing::debug!("chat line not recycled: {e}");
                    ui.destroy(first);
                    None
                }
            }
        } else {
            None
        };
        let line = match recycled {
            Some(k) => k,
            None => {
                // Step 5.
                let line = match ui.create_element(None, self.layout, tab.line_template) {
                    Ok(k) => k,
                    Err(e) => {
                        tracing::debug!("chat line not created: {e}");
                        return;
                    }
                };
                if ui.list_add(list, line, None).is_err() {
                    ui.destroy(line);
                    return;
                }
                line
            }
        };
        // Step 8.
        self.apply_style(ui, line, style.colour, self.styles.font(style.size_step));
        ui.set_text(line, Some(text.to_string()));
        // Step 10.
        if was_bottom {
            ui.scroll_to(list, offset.0, i32::MAX, false);
        }
        // Step 11.
        if let Some(did) = self.sounds.sound(mask) {
            ui.emit(UiEvent::Sound { did });
        }
        // Step 12.
        if !selected && let Some(u) = tab.unread {
            ui.set_shown(u, true);
        }
    }

    fn apply_style(&self, ui: &mut Ui, line: ElementKey, colour: u32, font: u32) {
        let p = self.vocab.pids;
        ui.set_property(
            line,
            p.text_font_color,
            PropertyValue::Colour(colour.to_le_bytes()),
        );
        if font == 0 {
            // A reused line drops the fonts of its earlier style.
            let fonts = ui.vocab().pids.text_fonts;
            for pid in [p.text_font, p.text_tag_font, fonts, p.text_tag_fonts] {
                if ui.el(line).is_some_and(|e| e.runtime.contains_key(&pid)) {
                    ui.clear_property(line, pid);
                }
            }
        } else {
            let d = PropertyValue::DidRef(Did(font));
            ui.set_property(line, p.text_font, d.clone());
            ui.set_property(line, p.text_tag_font, d.clone());
            let arr = PropertyValue::Array(PropertyArray {
                count: 1,
                elements: vec![PropertyEntry {
                    pid: p.text_font,
                    value: d,
                }],
            });
            let fonts = ui.vocab().pids.text_fonts;
            ui.set_property(line, fonts, arr.clone());
            ui.set_property(line, p.text_tag_fonts, arr);
        }
    }

    /// Applies a `GOP_ChatOpacity` value (§3.5 step 2): the table's image on every window's
    /// `LogFieldBackground` and the primary window's `ChatEntryBackGround`.
    pub fn apply_opacity(&self, ui: &mut Ui, value: i32) -> Option<u32> {
        let img = self.opacity.image(value)?;
        for w in &self.windows {
            if let Some(bg) = find_below(ui, w.root, self.vocab.ids.log_field_background) {
                set_image(ui, bg, img, OPACITY_DRAW_MODE);
            }
            if w.primary
                && let Some(bg) = find_below(ui, w.root, self.vocab.ids.entry_background)
            {
                set_image(ui, bg, img, OPACITY_DRAW_MODE);
            }
        }
        Some(img)
    }

    /// The primary window's index.
    #[must_use]
    pub fn primary(&self) -> Option<usize> {
        self.windows.iter().position(|w| w.primary)
    }

    fn entry(&self) -> Option<ElementKey> {
        self.primary().and_then(|p| self.windows[p].entry)
    }

    /// Whether the entry line has keyboard focus (§6.2).
    #[must_use]
    pub fn entry_focused(&self, ui: &Ui) -> bool {
        self.entry().is_some_and(|e| ui.focus_element() == Some(e))
    }

    fn set_entry_text(&self, ui: &mut Ui, text: &str) {
        if let Some(e) = self.entry() {
            ui.set_text(e, Some(text.to_string()));
            let units = text.encode_utf16().count();
            if let Some(el) = ui.el_mut(e) {
                el.text.caret = units;
            }
        }
    }

    fn restore_mode(&mut self, ui: &mut Ui) {
        if let Some(p) = self.primary() {
            let win = self.windows[p].clone();
            if let Some(t) = win.tabs.get(win.selected) {
                self.mode = t.mode;
            }
            self.update_label(ui, &win);
        }
    }

    /// The `OutputTextType` label (§6.5): the channel's display name, else (Tessera, see
    /// the implementation note under `chat-window.md` §6.5) the mode's command word.
    fn update_label(&self, ui: &mut Ui, w: &ChatWindow) {
        let Some(l) = w.output_label else { return };
        let name = self
            .texts
            .channel_names
            .get(&self.mode)
            .cloned()
            .unwrap_or_else(|| self.modes.command_word(self.mode).to_string());
        let text = if self.mode == TELL && !self.target.is_empty() {
            format!("{name} {}", self.target)
        } else {
            name
        };
        ui.set_text(l, Some(text));
    }

    /// The window the paging commands act on: the active chat window, else the primary.
    fn paging_window(&self, ui: &Ui) -> Option<usize> {
        let active = ui.active_panel();
        self.windows
            .iter()
            .position(|w| Some(w.root) == active)
            .or_else(|| self.primary())
    }

    /// Pages the shown log of a window (§5.5).
    pub fn page(&self, ui: &mut Ui, w: usize, command: &str) -> bool {
        let Some(win) = self.windows.get(w) else {
            return false;
        };
        let Some(list) = win.tabs.get(win.selected).and_then(|t| t.list) else {
            return false;
        };
        let (content, offset) = ui.scroll_state(list).unwrap_or(((0, 0), (0, 0)));
        let v = ui.layout_box(list).map_or(0, |b| b.h());
        let (prev, next) = page_targets(offset.1, v, content.1);
        match command {
            "CursorPreviousPage" => ui.scroll_to(list, offset.0, prev, false),
            "CursorNextPage" => ui.scroll_to(list, offset.0, next, false),
            "CursorStartOfDocument" => {
                if !ui.list_entries(list).is_empty() {
                    ui.scroll_to(list, offset.0, 0, false);
                }
            }
            "CursorEndOfDocument" => {
                if !ui.list_entries(list).is_empty() {
                    ui.scroll_to(list, offset.0, i32::MAX, false);
                }
            }
            _ => return false,
        }
        true
    }

    /// The chat window's handling of an input command (§6.2). `shift` and `ctrl` are the
    /// modifiers of the delivery (§6.4). Only presses are passed here.
    ///
    /// The chat window sees a command when it is the keyboard target (`ui-behaviour.md`
    /// §5.3): the client offers it here **first** (before [`Ui::key_command`]) while the
    /// entry line has focus or no element has focus, and offers a command that the UI
    /// declined and that went on to game code (decision 0014 D7, for example `open_chat`)
    /// here too. While another element has keyboard focus that element is the target: this
    /// method then returns [`CommandOutcome::NotHandled`] for every command, so that a
    /// focused plugin text box or editable text keeps its Enter and its paging keys.
    pub fn command(
        &mut self,
        ui: &mut Ui,
        name: &str,
        shift: bool,
        ctrl: bool,
        host: &mut dyn CommandHost,
    ) -> CommandOutcome {
        let focused = self.entry_focused(ui);
        let entry = self.entry();
        if !focused && ui.focus_element().is_some() {
            return CommandOutcome::NotHandled;
        }
        match name {
            "AcceptInput" => {
                let Some(e) = entry else {
                    return CommandOutcome::NotHandled;
                };
                if focused {
                    ui.set_focus(None);
                    let r = self.submit(ui, host);
                    return r.map_or(CommandOutcome::Handled, CommandOutcome::Submitted);
                }
                ui.set_focus(Some(e));
                CommandOutcome::Handled
            }
            "EscapeKey" => {
                if !focused {
                    return CommandOutcome::NotHandled;
                }
                ui.set_focus(None);
                self.set_entry_text(ui, "");
                self.restore_mode(ui);
                CommandOutcome::Handled
            }
            "START_COMMAND" | "START_ALIAS" => {
                let Some(e) = entry else {
                    return CommandOutcome::NotHandled;
                };
                ui.set_focus(Some(e));
                // Q10 Tessera rule: START_COMMAND opens with the prefix typed.
                let pre = if name == "START_COMMAND" {
                    self.command_line.setup.prefix.clone()
                } else {
                    String::new()
                };
                self.set_entry_text(ui, &pre);
                CommandOutcome::Handled
            }
            "ChatModeReply" => {
                if focused {
                    return CommandOutcome::NotHandled;
                }
                // The sandbox has no reply target (§10 item 8).
                let text = self.texts.no_reply_target.clone();
                self.post_line(ui, REPLY_MASK, &text);
                CommandOutcome::Handled
            }
            "CursorPreviousLine" | "CursorNextLine" => {
                if !focused {
                    return CommandOutcome::NotHandled;
                }
                if shift || ctrl {
                    // §6.4: game-script callbacks; Tessera makes these keys do nothing.
                    return CommandOutcome::Handled;
                }
                let t = if name == "CursorPreviousLine" {
                    self.command_line.history.older().map(str::to_string)
                } else {
                    self.command_line.history.newer().map(str::to_string)
                };
                if let Some(t) = t {
                    self.set_entry_text(ui, &t);
                }
                CommandOutcome::Handled
            }
            "CursorPreviousPage"
            | "CursorNextPage"
            | "CursorStartOfDocument"
            | "CursorEndOfDocument" => match self.paging_window(ui) {
                Some(w) if self.page(ui, w, name) => CommandOutcome::Handled,
                _ => CommandOutcome::NotHandled,
            },
            _ => CommandOutcome::NotHandled,
        }
    }

    /// Submit (§6.2): runs the entry text through the command line, posts the
    /// unknown-command line on result 3, clears the entry and restores the tab's mode.
    /// Returns the result, or `None` for an empty entry (nothing else changes).
    pub fn submit(&mut self, ui: &mut Ui, host: &mut dyn CommandHost) -> Option<i32> {
        let e = self.entry()?;
        let text = ui.text_of(e);
        if text.is_empty() {
            return None;
        }
        let word = self.modes.command_word(self.mode).to_string();
        let target = if self.mode == TELL {
            self.target.clone()
        } else {
            String::new()
        };
        let r = self.command_line.run_line(&text, &word, &target, host);
        if r == RESULT_UNKNOWN {
            let t = self.texts.unknown_command.clone();
            self.post_line(ui, ERROR_MASK, &t);
        }
        self.set_entry_text(ui, "");
        self.restore_mode(ui);
        self.last_result = Some(r);
        Some(r)
    }

    /// A typed character for the focused entry (§6.5, §6.6): the key-repeat guard (when on),
    /// the insertion through [`Ui::text_input`], then on a space the channel-mode switch.
    /// Without entry focus the character goes to [`Ui::text_input`] alone.
    ///
    /// On [`TextInputOutcome::GuardTripped`] the client plays UI sound
    /// [`super::entry::GUARD_SOUND`] (§6.6 step 3; which sound record that is, Q13, is
    /// OPEN: the UI core's sound event takes a sound-info DID, so the client resolves the
    /// UI sound ID or plays nothing).
    pub fn text_input(&mut self, ui: &mut Ui, ch: char, now: f64) -> TextInputOutcome {
        let taken = |t: bool| {
            if t {
                TextInputOutcome::Taken
            } else {
                TextInputOutcome::NotTaken
            }
        };
        let Some(e) = self.entry().filter(|_| self.entry_focused(ui)) else {
            return taken(ui.text_input(ch));
        };
        if let Some(g) = self.guard.as_mut()
            && let GuardVerdict::Trip { delete } = g.check(ch, now)
        {
            ui.set_focus(None);
            let text: Vec<u16> = ui.text_of(e).encode_utf16().collect();
            let caret = ui.el(e).map_or(0, |x| x.text.caret).min(text.len());
            let mut start = caret;
            let mut n = 0;
            while n < delete && start > 0 {
                start -= 1;
                if start > 0 && (0xDC00..=0xDFFF).contains(&text[start]) {
                    start -= 1;
                }
                n += 1;
            }
            let mut t2 = text.clone();
            t2.drain(start..caret);
            ui.set_text(e, Some(String::from_utf16_lossy(&t2)));
            if let Some(el) = ui.el_mut(e) {
                el.text.caret = start;
            }
            return TextInputOutcome::GuardTripped;
        }
        let t = ui.text_input(ch);
        if ch == ' ' {
            self.mode_switch(ui, e);
        }
        taken(t)
    }

    fn mode_switch(&mut self, ui: &mut Ui, e: ElementKey) {
        let text = ui.text_of(e);
        let Some(row) = self.modes.switch_word(&text).cloned() else {
            return;
        };
        // Step 3: a reply or tell-again word asks a callback for the target; the sandbox
        // has none, so the target is unchanged (§6.4 Q11).
        self.previous_mode = self.mode;
        self.mode = row.chat_type;
        if let Some(p) = self.primary() {
            let sel = self.windows[p].selected;
            if let Some(t) = self.windows[p].tabs.get_mut(sel) {
                t.mode = self.mode;
            }
            let win = self.windows[p].clone();
            self.update_label(ui, &win);
        }
        self.set_entry_text(ui, "");
    }

    /// Routes a UI event: a tab click selects the tab (`ListBox_new_selection` from a tab
    /// list).
    pub fn handle_event(&mut self, ui: &mut Ui, ev: &UiEvent) {
        let UiEvent::Game { message } = ev else {
            return;
        };
        self.game_message(ui, message);
    }

    fn game_message(&mut self, ui: &mut Ui, m: &Message) {
        if m.msg != ui.vocab().msgs.listbox_new_selection {
            return;
        }
        let Some(w) = self
            .windows
            .iter()
            .position(|w| w.tab_list == Some(m.source))
        else {
            return;
        };
        let Some(sel) = ui.list_selected(m.source) else {
            return;
        };
        if let Some(i) = self.windows[w].tabs.iter().position(|t| t.tab == sel) {
            self.select_tab(ui, w, i);
        }
    }
}
