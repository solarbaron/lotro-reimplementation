// SPDX-License-Identifier: GPL-3.0-or-later
//! UI message delivery (`ui-behaviour.md` §12.1) and the listener registry
//! (`ui-layout.md` §9.2 step 5, `ui-behaviour.md` §4.3 item 2).

use crate::events::{Answer, ControlEvent, Message, MsgArg, UiEvent};
use crate::tree::ElementKey;
use crate::ui::{MAX_TREE_DEPTH, Ui};
use crate::vocab::tessera_msg;

/// Most messages waiting in the queue (Tessera cap; a handler loop that keeps posting is cut
/// with a warning).
pub const MAX_QUEUED: usize = 1 << 16;

/// Most messages delivered in one drain (Tessera cap against handler loops).
pub const MAX_DRAIN: usize = 1 << 18;

impl Ui {
    /// Posts a message from `source` (`ui-behaviour.md` §12.1). When no delivery is running
    /// it is delivered now and its overall answer returned; otherwise it is queued and
    /// `None` returned.
    pub fn post(&mut self, source: ElementKey, msg: u32, arg: MsgArg) -> Option<Answer> {
        let source_id = self.el(source)?.element_id;
        self.seq += 1;
        let m = Message {
            seq: self.seq,
            source,
            source_id,
            msg,
            arg,
        };
        if self.delivering {
            if self.queue.len() < MAX_QUEUED {
                self.queue.push_back(m);
            } else {
                tracing::warn!("UI message queue full; message {msg:#x} dropped");
            }
            return None;
        }
        self.delivering = true;
        let answer = self.deliver(&m);
        let mut n = 0;
        while let Some(q) = self.queue.pop_front() {
            n += 1;
            if n > MAX_DRAIN {
                tracing::warn!("UI message drain cut after {MAX_DRAIN} messages");
                self.queue.clear();
                break;
            }
            self.deliver(&q);
        }
        self.delivering = false;
        Some(answer)
    }

    /// Delivers one message up the source's chain (§12.1 step 2), then to the game-code
    /// handlers at the top (step 3). Returns Stop, Handled or NotHandled for the whole walk.
    fn deliver(&mut self, m: &Message) -> Answer {
        let mut overall = Answer::NotHandled;
        let mut seen: Vec<ElementKey> = Vec::new();
        let mut x = Some(m.source);
        let mut depth = 0;
        while let Some(cur) = x {
            if !self.exists(cur) {
                break;
            }
            depth += 1;
            if depth > MAX_TREE_DEPTH * 2 {
                break;
            }
            if !seen.contains(&cur) {
                seen.push(cur);
                let a = self.native_handle(cur, m);
                if a == Answer::Stop {
                    return Answer::Stop;
                }
                overall = overall.max(a);
                // The plugin listener: an event only when the control is the source.
                if cur == m.source {
                    self.plugin_listener(cur, m);
                }
                let listeners = self
                    .el(cur)
                    .map(|e| e.listeners.clone())
                    .unwrap_or_default();
                for l in listeners {
                    if seen.contains(&l) {
                        continue;
                    }
                    seen.push(l);
                    let a = self.listener_handle(l, cur, m);
                    if a == Answer::Stop {
                        return Answer::Stop;
                    }
                    overall = overall.max(a);
                }
            }
            x = self.parent(cur);
        }
        // Step 3 with its Tessera rule: game-code handlers run unless stopped.
        if self.registry.contains_key(&(m.source_id, m.msg)) {
            self.emit(UiEvent::Game { message: m.clone() });
        }
        overall
    }

    /// Registers a game-code handler for (element ID, message) in the listener registry
    /// (§12.1 step 3). For the four mouse-button messages every live element with that ID
    /// becomes mouse-visible (`ui-behaviour.md` §4.3 item 2).
    pub fn register_handler(&mut self, element_id: u32, msg: u32) {
        *self.registry.entry((element_id, msg)).or_insert(0) += 1;
        let ms = self.v.msgs;
        if [
            ms.mouse_release,
            ms.mouse_tap,
            ms.mouse_click,
            ms.mouse_press,
        ]
        .contains(&msg)
        {
            for k in self.arena.keys() {
                if let Some(e) = self.el_mut(k)
                    && e.element_id == element_id
                {
                    e.mouse_visible = true;
                }
            }
            self.input.hover_dirty = true;
        }
    }

    /// Removes one registration made by [`Ui::register_handler`].
    pub fn unregister_handler(&mut self, element_id: u32, msg: u32) {
        if let Some(n) = self.registry.get_mut(&(element_id, msg)) {
            *n -= 1;
            if *n == 0 {
                self.registry.remove(&(element_id, msg));
            }
        }
    }

    /// Whether the registry holds a mouse-button handler for an element ID (used at creation,
    /// `ui-layout.md` §9.2 step 5).
    pub(crate) fn registry_has_mouse(&self, element_id: u32) -> bool {
        let ms = self.v.msgs;
        [
            ms.mouse_release,
            ms.mouse_tap,
            ms.mouse_click,
            ms.mouse_press,
        ]
        .iter()
        .any(|m| self.registry.contains_key(&(element_id, *m)))
    }

    /// Attaches `listener` to `target`: it receives the messages delivered to `target`
    /// (§12.1 step 2.2).
    pub(crate) fn attach_listener(&mut self, target: ElementKey, listener: ElementKey) {
        if let Some(e) = self.el_mut(target)
            && !e.listeners.contains(&listener)
            && e.listeners.len() < 1024
        {
            e.listeners.push(listener);
        }
    }

    /// The plugin control's listener (`lua-plugin-api.md` §9.7): turns a notification whose
    /// source is the control into an event.
    fn plugin_listener(&mut self, k: ElementKey, m: &Message) {
        if self.el(k).is_none_or(|e| e.plugin.is_none()) {
            return;
        }
        let ms = self.v.msgs;
        let a = &m.arg;
        let button = a.command.as_ref().map_or(0, |c| c.mouse_button());
        let (x, y) = a.pos;
        let ev = match m.msg {
            v if v == ms.mouse_over => {
                if a.value != 0 {
                    ControlEvent::MouseEnter
                } else {
                    ControlEvent::MouseLeave
                }
            }
            v if v == ms.mouse_press => ControlEvent::MouseDown { button, x, y },
            v if v == ms.mouse_release => ControlEvent::MouseUp { button, x, y },
            v if v == ms.mouse_click => ControlEvent::MouseClick { button, x, y },
            v if v == ms.mouse_dbl_click => ControlEvent::MouseDoubleClick { button, x, y },
            v if v == ms.mouse_move => ControlEvent::MouseMove { x, y },
            v if v == ms.mouse_hover => ControlEvent::MouseHover { x, y },
            v if v == ms.mouse_scroll => {
                let direction = match a.command.as_ref().map(|c| &*c.name) {
                    Some(crate::vocab::commands::SCROLL_UP) => 1,
                    Some(crate::vocab::commands::SCROLL_DOWN) => -1,
                    _ => return,
                };
                ControlEvent::MouseWheel { direction, x, y }
            }
            v if v == ms.set_focus => {
                if a.value != 0 {
                    ControlEvent::FocusGained
                } else {
                    ControlEvent::FocusLost
                }
            }
            v if v == ms.activated => {
                if !self.el(k).is_some_and(|e| e.root_panel) {
                    return;
                }
                ControlEvent::Activated
            }
            v if v == ms.deactivated => {
                if !self.el(k).is_some_and(|e| e.root_panel) {
                    return;
                }
                ControlEvent::Deactivated
            }
            v if v == ms.key_press || v == ms.key_release => {
                // Only a text box fires these from its own notification (§13.13 step 2).
                let is_box = self
                    .el(k)
                    .and_then(|e| e.plugin.as_ref())
                    .is_some_and(|p| p.kind == crate::control::ControlKind::TextBox);
                let Some(command) = a.command.clone() else {
                    return;
                };
                if !is_box {
                    return;
                }
                if v == ms.key_press {
                    ControlEvent::KeyDown { command }
                } else {
                    ControlEvent::KeyUp { command }
                }
            }
            tessera_msg::SIZE_CHANGED => ControlEvent::SizeChanged,
            tessera_msg::POSITION_CHANGED => ControlEvent::PositionChanged,
            tessera_msg::VISIBLE_CHANGED => ControlEvent::VisibleChanged,
            tessera_msg::CONTROL_CLICK => ControlEvent::Click,
            tessera_msg::TEXT_CHANGED => ControlEvent::TextChanged,
            _ => return,
        };
        self.emit(UiEvent::Control {
            control: k,
            event: ev,
        });
    }
}
