// SPDX-License-Identifier: GPL-3.0-or-later
//! Activation and z-order (`ui-behaviour.md` §5.1), keyboard focus and the input-map stack
//! (§5.2, decision 0014 D5), key routing (§5.3), text-entry keys (§5.4), dialog keys (§5.5)
//! and keyboard navigation (§5.6, Tessera rule).

use std::collections::BTreeMap;

use crate::events::{Answer, Command, MsgArg, UiEvent};
use crate::tree::ElementKey;
use crate::ui::Ui;
use crate::vocab::{commands, input_maps};
use crate::widgets::WidgetClass;

/// One entry of the UI's map stack: (InputMap name, owner, priority) (§5.2 step 2).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BandEntry {
    /// InputMap name (or the Tessera context name `ui.chat_log`).
    pub map: String,
    /// The element the entry belongs to.
    pub owner: ElementKey,
    /// Priority; higher first.
    pub priority: i32,
}

/// A push or pop for the client to turn into a band request (decision 0014 D5, "The
/// request channel").
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BandChange {
    /// Push (map, owner, priority).
    Push {
        /// InputMap name.
        map: String,
        /// Owner ([`ElementKey::owner`], never 0).
        owner: u64,
        /// Priority.
        priority: i32,
    },
    /// Pop (map, owner) at every priority.
    Pop {
        /// InputMap name.
        map: String,
        /// Owner.
        owner: u64,
    },
}

/// Most entries the UI keeps in its band mirror (decision 0014 D5: the band holds 256).
pub const MAX_BAND: usize = 256;

/// Focus and activation state.
#[derive(Debug, Clone, Default)]
pub(crate) struct FocusState {
    pub active: Option<ElementKey>,
    pub activation_list: Vec<ElementKey>,
    pub focus: Option<ElementKey>,
    pub band: Vec<BandEntry>,
    pub outbox: Vec<BandChange>,
    /// Extra maps an element pushes as its own (engine dialogs: `DialogBoxes`; the chat
    /// window: `ui.chat_log`).
    pub extra_maps: BTreeMap<ElementKey, Vec<String>>,
    /// Engine dialogs (§5.5).
    pub dialogs: Vec<ElementKey>,
    /// Activation guard (`lua-plugin-api.md` §13.14).
    pub activating: Option<ElementKey>,
}

impl Ui {
    // ----- band (§5.2 step 2) ------------------------------------------------------------

    /// The UI's record of its band, in stack order (highest priority first).
    #[must_use]
    pub fn band(&self) -> &[BandEntry] {
        &self.focus.band
    }

    /// Takes the recorded pushes and pops (decision 0014 D5 outbox).
    pub fn take_band_changes(&mut self) -> Vec<BandChange> {
        std::mem::take(&mut self.focus.outbox)
    }

    fn band_add(&mut self, map: &str, owner: ElementKey, priority: i32) {
        let e = BandEntry {
            map: map.to_string(),
            owner,
            priority,
        };
        if self.focus.band.contains(&e) {
            return;
        }
        if self.focus.band.len() >= MAX_BAND {
            tracing::warn!("UI band full; push of {map} refused");
            return;
        }
        let pos = self
            .focus
            .band
            .iter()
            .position(|x| x.priority <= priority)
            .unwrap_or(self.focus.band.len());
        self.focus.band.insert(pos, e);
        self.focus.outbox.push(BandChange::Push {
            map: map.to_string(),
            owner: owner.owner(),
            priority,
        });
    }

    fn band_remove(&mut self, map: &str, owner: ElementKey) {
        let before = self.focus.band.len();
        self.focus
            .band
            .retain(|x| !(x.map == map && x.owner == owner));
        if self.focus.band.len() != before {
            self.focus.outbox.push(BandChange::Pop {
                map: map.to_string(),
                owner: owner.owner(),
            });
        }
    }

    /// Pushes one map for `owner` (the hover pushes of `ui-widgets.md` §5.7, §6.9).
    /// Tessera: the entry alone, without the ancestor walk (no ancestor of a hovered
    /// element is popped when the hover ends).
    pub(crate) fn band_push_map(&mut self, owner: ElementKey, map: &str, priority: i32) {
        self.band_add(map, owner, priority);
    }

    /// Pops one map of `owner`.
    pub(crate) fn band_pop_map(&mut self, owner: ElementKey, map: &str) {
        self.band_remove(map, owner);
    }

    /// Removes every entry of `k` (the element is destroyed).
    pub(crate) fn band_pop_all(&mut self, k: ElementKey) {
        let maps: Vec<String> = self
            .focus
            .band
            .iter()
            .filter(|e| e.owner == k)
            .map(|e| e.map.clone())
            .collect();
        for m in maps {
            self.band_remove(&m, k);
        }
    }

    /// The maps an element pushes as its own (§5.2 step 2 with the Tessera rules of IB D5).
    fn element_maps(&mut self, k: ElementKey) -> Vec<String> {
        let mut v = Vec::new();
        let pid = self.v.pids.input_map_id;
        let id = self.prop(k, pid);
        if id.is_set() {
            let key = id.key();
            if let Some(n) = self.env.input_map_name(key) {
                v.push(n);
            }
        }
        if let Some(x) = self.focus.extra_maps.get(&k) {
            v.extend(x.iter().cloned());
        }
        if self.focus.focus == Some(k) && self.is_editable(k) {
            v.push(input_maps::EDIT.to_string());
            v.push(input_maps::CLIPBOARD.to_string());
        }
        v
    }

    fn push_maps(&mut self, k: ElementKey, priority: i32) {
        let mut chain = Vec::new();
        let mut cur = Some(k);
        while let Some(c) = cur {
            if chain.len() > crate::ui::MAX_TREE_DEPTH * 2 {
                break;
            }
            chain.push(c);
            cur = self.parent(c);
        }
        // The topmost ancestor pushes first, one priority lower per level.
        for (i, c) in chain.iter().enumerate().rev() {
            let p = priority.saturating_sub(i as i32);
            for m in self.element_maps(*c) {
                self.band_add(&m, *c, p);
            }
        }
    }

    fn pop_maps(&mut self, k: ElementKey, own: Vec<String>) {
        let mut chain = Vec::new();
        let mut cur = self.parent(k);
        while let Some(c) = cur {
            if chain.len() > crate::ui::MAX_TREE_DEPTH * 2 {
                break;
            }
            chain.push(c);
            cur = self.parent(c);
        }
        for c in chain.iter().rev() {
            for m in self.element_maps(*c) {
                self.band_remove(&m, *c);
            }
        }
        for m in own {
            self.band_remove(&m, k);
        }
    }

    /// Registers extra maps an element pushes as its own when it pushes (Tessera: the chat
    /// window's `ui.chat_log`).
    pub fn set_element_maps(&mut self, k: ElementKey, maps: Vec<String>) {
        if maps.is_empty() {
            self.focus.extra_maps.remove(&k);
        } else {
            self.focus.extra_maps.insert(k, maps);
        }
    }

    /// Marks a panel as an engine dialog (`ui-behaviour.md` §5.5): it pushes `DialogBoxes`
    /// while it is the active panel (decision 0014 D5).
    pub fn mark_dialog(&mut self, k: ElementKey) {
        if !self.focus.dialogs.contains(&k) {
            self.focus.dialogs.push(k);
        }
        let maps = self.focus.extra_maps.entry(k).or_default();
        if !maps.iter().any(|m| m == input_maps::DIALOG) {
            maps.push(input_maps::DIALOG.to_string());
        }
    }

    // ----- activation (§5.1) -------------------------------------------------------------

    /// The active panel.
    #[must_use]
    pub fn active_panel(&self) -> Option<ElementKey> {
        self.focus.active
    }

    /// The focus element.
    #[must_use]
    pub fn focus_element(&self) -> Option<ElementKey> {
        self.focus.focus
    }

    /// Activates the nearest root panel at or above `k` (§5.1 step 3).
    pub fn activate(&mut self, k: ElementKey) {
        let Some(p) = self.root_panel_of(k) else {
            return;
        };
        if self.focus.active == Some(p) || self.focus.activating == Some(p) {
            return;
        }
        self.focus.activating = Some(p);
        self.deactivate_active();
        self.focus.active = Some(p);
        self.push_maps(p, 0);
        self.focus.activation_list.retain(|x| *x != p);
        self.focus.activation_list.push(p);
        self.bring_to_front(p);
        self.post(p, self.v.msgs.activated, MsgArg::value(1));
        let remembered = self
            .el(p)
            .and_then(|e| e.plugin.as_ref())
            .and_then(|pd| pd.focus_control);
        if let Some(f) = remembered.filter(|f| self.exists(*f)) {
            self.set_focus(Some(f));
        }
        self.focus.activating = None;
    }

    /// Activation requested by code or by `activate_on_show` (§5.1 step 2): the panel must be
    /// displayed and not active; a panel that is not activatable becomes activatable.
    pub fn request_activation(&mut self, k: ElementKey) {
        let Some(p) = self.root_panel_of(k) else {
            return;
        };
        if !self.is_displayed(p) || self.focus.active == Some(p) {
            return;
        }
        let pid = self.v.pids.activatable;
        if !self.prop(p, pid).bool() {
            self.set_property(p, pid, tessera_formats::property::PropertyValue::Bool(true));
        }
        self.activate(p);
    }

    /// Deactivates the active panel (§5.1 step 3, first bullet).
    pub(crate) fn deactivate_active(&mut self) {
        let Some(old) = self.focus.active.take() else {
            return;
        };
        let maps = if self.exists(old) {
            self.element_maps(old)
        } else {
            Vec::new()
        };
        self.pop_maps(old, maps);
        // Order: `Deactivated` before the focus loss, as the worked example of
        // `lua-plugin-api.md` §13.13 gives it (UB §5.1 lists the focus loss first; OPEN there).
        if self.exists(old) {
            self.post(old, self.v.msgs.deactivated, MsgArg::value(0));
        }
        if self.focus.focus.is_some() {
            self.set_focus_inner(None, false);
        }
    }

    /// The hidden active panel (§5.1 step 6): deactivate it and activate the most recently
    /// activated panel still displayed.
    pub(crate) fn active_hidden(&mut self, k: ElementKey) {
        self.focus.activation_list.retain(|x| *x != k);
        if self.focus.active == Some(k) {
            self.deactivate_active();
            let next = self
                .focus
                .activation_list
                .iter()
                .rev()
                .copied()
                .find(|p| self.is_displayed(*p));
            if let Some(n) = next {
                self.activate(n);
            }
        }
        if let Some(f) = self.focus.focus
            && (f == k || self.is_ancestor_or_self(k, f))
        {
            self.set_focus(None);
        }
    }

    pub(crate) fn focus_forget_set(&mut self, doomed: &std::collections::HashSet<ElementKey>) {
        if self.focus.focus.is_some_and(|f| doomed.contains(&f)) {
            self.focus.focus = None;
        }
        if self.focus.active.is_some_and(|a| doomed.contains(&a)) {
            self.focus.active = None;
        }
        self.focus.activation_list.retain(|x| !doomed.contains(x));
        self.focus.extra_maps.retain(|x, _| !doomed.contains(x));
        self.focus.dialogs.retain(|x| !doomed.contains(x));
    }

    // ----- focus (§5.2) ------------------------------------------------------------------

    /// Gives keyboard focus to `f`, or clears it (§5.2 step 1).
    pub fn set_focus(&mut self, f: Option<ElementKey>) {
        self.set_focus_inner(f, true);
    }

    fn set_focus_inner(&mut self, f: Option<ElementKey>, restore_active: bool) {
        if self.focus.focus == f {
            return;
        }
        if let Some(old) = self.focus.focus {
            let maps = if self.exists(old) {
                self.element_maps(old)
            } else {
                Vec::new()
            };
            self.focus.focus = None;
            if self.exists(old) {
                self.pop_maps(old, maps);
                self.post(old, self.v.msgs.set_focus, MsgArg::value(0));
            }
        }
        match f.filter(|x| self.exists(*x)) {
            Some(nf) => {
                self.focus.focus = Some(nf);
                self.push_maps(nf, 50);
                self.post(nf, self.v.msgs.set_focus, MsgArg::value(1));
            }
            None => {
                self.focus.focus = None;
                if restore_active && let Some(a) = self.focus.active {
                    self.push_maps(a, 0);
                }
            }
        }
    }

    /// Whether an element edits text (an editable Text, or a plugin `TextBox`).
    pub(crate) fn is_editable(&self, k: ElementKey) -> bool {
        let Some(e) = self.el(k) else { return false };
        if let Some(pd) = e.plugin.as_ref() {
            return pd.kind == crate::control::ControlKind::TextBox;
        }
        e.class == WidgetClass::Text && self.prop(k, self.v.pids.text_editable).bool()
    }

    // ----- keys (§5.3–§5.6) --------------------------------------------------------------

    /// The keyboard target (§5.3 step 1).
    #[must_use]
    pub fn keyboard_target(&self) -> Option<ElementKey> {
        self.focus.focus.or(self.focus.active)
    }

    /// Routes a key command (§5.3). `down` is a press (or an OS repeat), otherwise a
    /// release. Returns whether it is consumed; when it is not, a [`UiEvent::KeyBroadcast`]
    /// goes to the outbox (the plugins' `KeyDown`/`KeyUp` broadcast and game code).
    pub fn key_command(&mut self, c: &Command, down: bool, value: f32) -> bool {
        if self.drag.swallows_keys() && self.keyboard_target().is_some() {
            return true;
        }
        let mut consumed = false;
        if let Some(t) = self.keyboard_target().filter(|t| self.exists(*t)) {
            if down {
                consumed = self.key_handler(t, c);
                let a = self.post(
                    t,
                    self.v.msgs.key_press,
                    MsgArg {
                        command: Some(c.clone()),
                        value: i64::from(crate::geom::trunc_f32(value * 1000.0)),
                        ..MsgArg::default()
                    },
                );
                consumed |= a.is_some_and(|a| a >= Answer::Handled);
            } else {
                consumed = self.key_release_handler(t, c);
                let a = self.post(
                    t,
                    self.v.msgs.key_release,
                    MsgArg {
                        command: Some(c.clone()),
                        ..MsgArg::default()
                    },
                );
                consumed |= a.is_some_and(|a| a >= Answer::Handled);
            }
        }
        if !consumed {
            self.emit(UiEvent::KeyBroadcast {
                down,
                command: c.clone(),
            });
        }
        consumed
    }

    /// The target's key handler on a press: text entry (§5.4), dialogs (§5.5), keyboard
    /// navigation (§5.6 Tessera rule).
    fn key_handler(&mut self, t: ElementKey, c: &Command) -> bool {
        if self.focus.focus == Some(t) && self.is_editable(t) && self.text_key(t, c) {
            return true;
        }
        let dialog = self
            .root_panel_of(t)
            .filter(|d| self.focus.dialogs.contains(d));
        if let Some(d) = dialog
            && self.focus.active == Some(d)
            && !self.prop(d, self.v.pids.dialog_ignore_keyboard).bool()
        {
            match &*c.name {
                commands::ACCEPT_KEY | commands::ACCEPT_INPUT => {
                    self.dialog_close(d, true);
                    return true;
                }
                commands::ESCAPE => {
                    self.dialog_close(d, false);
                    return true;
                }
                commands::FOCUS_NEXT | commands::FOCUS_PREVIOUS => {
                    self.focus_cycle(d, &*c.name == commands::FOCUS_NEXT);
                    return true;
                }
                _ => {}
            }
        }
        false
    }

    fn key_release_handler(&mut self, t: ElementKey, c: &Command) -> bool {
        let dialog = self
            .root_panel_of(t)
            .filter(|d| self.focus.dialogs.contains(d));
        dialog.is_some()
            && matches!(
                &*c.name,
                commands::ACCEPT_KEY | commands::ACCEPT_INPUT | commands::ESCAPE
            )
    }

    /// Closes an engine dialog with the result of its yes (`true`) or no button (§5.5).
    pub fn dialog_close(&mut self, d: ElementKey, yes: bool) {
        self.emit(UiEvent::DialogResult { dialog: d, yes });
        self.set_shown(d, false);
    }

    /// Tessera rule (§5.6 OPEN): cycle through the displayed, focusable elements of the
    /// panel in read order, depth first.
    fn focus_cycle(&mut self, panel: ElementKey, forward: bool) {
        let mut list = Vec::new();
        let mut stack = vec![panel];
        while let Some(c) = stack.pop() {
            if list.len() > 1 << 14 {
                break;
            }
            if c != panel && self.is_displayed(c) && self.is_editable(c) {
                list.push(c);
            }
            let mut kids = self.children(c);
            kids.sort_by_key(|k| self.el(*k).map_or(0, |e| e.read_order));
            for k in kids.into_iter().rev() {
                stack.push(k);
            }
        }
        if list.is_empty() {
            return;
        }
        let cur = self
            .focus
            .focus
            .and_then(|f| list.iter().position(|x| *x == f));
        let n = list.len();
        let next = match (cur, forward) {
            (None, true) => 0,
            (None, false) => n - 1,
            (Some(i), true) => (i + 1) % n,
            (Some(i), false) => (i + n - 1) % n,
        };
        self.set_focus(Some(list[next]));
    }

    /// The text-entry keys (§5.4) of a focused editable element. Returns whether consumed.
    ///
    /// The caret counts UTF-16 units (`lua-plugin-api.md` §13 P71), but it moves over and
    /// deletes whole characters: a surrogate pair is never split. Tessera rule (§5.4 OPEN):
    /// without a selection model the whole text is the selection of `CopyText` and
    /// `CutText`; `PasteText` asks the client for the clipboard ([`UiEvent::PasteRequest`],
    /// answered with [`Ui::paste_text`]).
    fn text_key(&mut self, t: ElementKey, c: &Command) -> bool {
        let p = self.v.pids;
        let editable = self.is_editable(t);
        let selectable = self.prop(t, p.text_selectable).bool();
        let plugin = self.el(t).is_some_and(|e| e.plugin.is_some());
        let text: Vec<u16> = self.text_of(t).encode_utf16().collect();
        let caret = snap_caret(&text, self.el(t).map_or(0, |e| e.text.caret));
        let set_caret = |ui: &mut Ui, v: usize| {
            if let Some(e) = ui.el_mut(t) {
                e.text.caret = v;
                e.dirty = true;
            }
        };
        match &*c.name {
            commands::CURSOR_CHAR_LEFT => {
                set_caret(self, char_left(&text, caret));
                true
            }
            commands::CURSOR_CHAR_RIGHT => {
                set_caret(self, char_right(&text, caret));
                true
            }
            commands::CURSOR_START_OF_LINE | commands::CURSOR_START_OF_DOCUMENT => {
                set_caret(self, 0);
                true
            }
            commands::CURSOR_END_OF_LINE | commands::CURSOR_END_OF_DOCUMENT => {
                set_caret(self, text.len());
                true
            }
            "CursorWordLeft" | "CursorWordRight" | "CursorPreviousLine" | "CursorNextLine"
            | "CursorPreviousPage" | "CursorNextPage" => true,
            commands::COPY => {
                self.emit(UiEvent::CopyText {
                    text: String::from_utf16_lossy(&text),
                });
                true
            }
            commands::CUT if editable && (selectable || plugin) => {
                self.emit(UiEvent::CopyText {
                    text: String::from_utf16_lossy(&text),
                });
                if !text.is_empty() {
                    self.set_text_edited(t, String::new());
                }
                set_caret(self, 0);
                true
            }
            commands::CUT => false,
            commands::PASTE => {
                if editable {
                    self.emit(UiEvent::PasteRequest { target: t });
                }
                true
            }
            commands::DELETE if editable => {
                let end = char_right(&text, caret);
                if end > caret {
                    let mut t2 = text.clone();
                    t2.drain(caret..end);
                    self.set_text_edited(t, String::from_utf16_lossy(&t2));
                    set_caret(self, caret);
                }
                true
            }
            commands::BACKSPACE if editable => {
                let start = char_left(&text, caret);
                if start < caret {
                    let mut t2 = text.clone();
                    t2.drain(start..caret);
                    self.set_text_edited(t, String::from_utf16_lossy(&t2));
                    set_caret(self, start);
                }
                true
            }
            commands::ESCAPE if !plugin && self.prop(t, p.text_lose_focus_on_escape).bool() => {
                self.set_focus(None);
                true
            }
            commands::ACCEPT_INPUT
                if !plugin && self.prop(t, p.text_lose_focus_on_accept).bool() =>
            {
                self.set_focus(None);
                true
            }
            _ => false,
        }
    }

    /// Inserts `units` at the caret of the editable element `t` when they fit under
    /// `UICore_Text_max_chars` (UTF-16 units, Tessera reading) and the text cap. Returns
    /// whether they were inserted.
    fn insert_units(&mut self, t: ElementKey, units: &[u16]) -> bool {
        let mut text: Vec<u16> = self.text_of(t).encode_utf16().collect();
        let max = self.prop(t, self.v.pids.text_max_chars);
        let after = text.len().saturating_add(units.len());
        if after > crate::text::MAX_TEXT_UNITS
            || (max.is_set() && max.int() >= 0 && after > max.int() as usize)
        {
            return false;
        }
        let caret = snap_caret(&text, self.el(t).map_or(0, |e| e.text.caret));
        text.splice(caret..caret, units.iter().copied());
        self.set_text_edited(t, String::from_utf16_lossy(&text));
        if let Some(e) = self.el_mut(t) {
            e.text.caret = caret + units.len();
        }
        true
    }

    /// The client's answer to [`UiEvent::PasteRequest`]: inserts `clip` at the caret of the
    /// editable element `target` (decision 0014 D7 lets the client answer in the same
    /// step). Tessera rule (`ui-behaviour.md` §5.4 OPEN): characters go in as typed
    /// characters would, control characters are dropped, and the first character that does
    /// not fit under `max_chars` ends the paste. Returns the number of characters inserted.
    pub fn paste_text(&mut self, target: ElementKey, clip: &str) -> usize {
        if !self.is_editable(target) {
            return 0;
        }
        let mut n = 0;
        for ch in clip.chars().filter(|c| !c.is_control()) {
            let mut buf = [0u16; 2];
            if !self.insert_units(target, ch.encode_utf16(&mut buf)) {
                break;
            }
            n += 1;
        }
        n
    }

    /// A typed character (decision 0014 D6–D7): inserted into the focused editable element.
    /// Returns whether it was taken.
    pub fn text_input(&mut self, ch: char) -> bool {
        let Some(t) = self.focus.focus.filter(|t| self.is_editable(*t)) else {
            return false;
        };
        if ch.is_control() {
            return true;
        }
        // `UICore_Text_max_chars` limits typed characters (§5.4; counted in UTF-16 units,
        // Tessera reading): a character that does not fit is refused whole.
        let mut buf = [0u16; 2];
        if self.insert_units(t, ch.encode_utf16(&mut buf)) {
            self.post(
                t,
                self.v.msgs.character_input,
                MsgArg::value(i64::from(u32::from(ch))),
            );
        }
        true
    }
}

fn is_high_surrogate(u: u16) -> bool {
    (0xD800..=0xDBFF).contains(&u)
}

fn is_low_surrogate(u: u16) -> bool {
    (0xDC00..=0xDFFF).contains(&u)
}

/// The caret clamped to the text and moved off the middle of a surrogate pair.
fn snap_caret(text: &[u16], caret: usize) -> usize {
    let c = caret.min(text.len());
    if c > 0 && c < text.len() && is_low_surrogate(text[c]) && is_high_surrogate(text[c - 1]) {
        c - 1
    } else {
        c
    }
}

/// The caret one character to the left (a surrogate pair is one character).
fn char_left(text: &[u16], c: usize) -> usize {
    if c == 0 {
        0
    } else if c >= 2 && is_low_surrogate(text[c - 1]) && is_high_surrogate(text[c - 2]) {
        c - 2
    } else {
        c - 1
    }
}

/// The caret one character to the right (a surrogate pair is one character).
fn char_right(text: &[u16], c: usize) -> usize {
    if c >= text.len() {
        text.len()
    } else if c + 1 < text.len() && is_high_surrogate(text[c]) && is_low_surrogate(text[c + 1]) {
        c + 2
    } else {
        c + 1
    }
}
