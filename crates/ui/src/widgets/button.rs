// SPDX-License-Identifier: GPL-3.0-or-later
//! Button behaviour: hover and press flags for the state choice (`ui-layout.md` §9.7),
//! auto-repeat, clicks and the boolean toggle (`ui-widgets.md` §7.1).

use tessera_formats::property::PropertyValue;

use crate::events::{Answer, Command, Message, MsgArg};
use crate::tree::ElementKey;
use crate::ui::Ui;
use crate::vocab::{commands, tessera_msg};

impl Ui {
    /// The Button's handling of its own messages. Returns its answer.
    pub(crate) fn button_message(&mut self, k: ElementKey, m: &Message) -> Answer {
        let ms = self.v.msgs;
        if m.source != k {
            return Answer::NotHandled;
        }
        if m.msg == ms.mouse_over_top {
            if let Some(b) = self.el_mut(k).and_then(|e| e.widget.button.as_mut()) {
                b.over = m.arg.value != 0;
            }
            if self
                .el(k)
                .is_some_and(|e| e.class == crate::widgets::WidgetClass::Menu)
            {
                // §11.3: `Element_mouse_over_top` sets `highlighted` to the open mark.
                let open = self
                    .el(k)
                    .and_then(|e| e.widget.menu.as_ref())
                    .is_some_and(|mp| mp.open);
                let pid = self.v.pids.button_highlighted;
                self.set_property(k, pid, PropertyValue::Bool(open));
            }
            self.button_choose_state(k);
            return Answer::NotHandled;
        }
        let Some(cmd) = m.arg.command.clone() else {
            return Answer::NotHandled;
        };
        let select = &*cmd.name == commands::SELECT_LEFT || &*cmd.name == commands::SELECT_DBL_LEFT;
        if m.msg == ms.mouse_press && select {
            self.button_press(k, cmd);
            return Answer::NotHandled;
        }
        if m.msg == ms.mouse_release {
            let pressing = self
                .el(k)
                .and_then(|e| e.widget.button.as_ref())
                .and_then(|b| b.pressed_by.clone());
            if pressing.as_deref() == Some(&*cmd.name) {
                self.button_release(k);
            }
        }
        Answer::NotHandled
    }

    fn button_press(&mut self, k: ElementKey, cmd: Command) {
        let p = self.v.pids;
        if let Some(b) = self.el_mut(k).and_then(|e| e.widget.button.as_mut()) {
            b.pressed_by = Some(cmd.name.clone());
        }
        let ghosted = self.prop(k, p.button_ghosted).bool();
        if (!ghosted || self.prop(k, p.button_broadcast_when_ghosted).bool())
            && self.prop(k, p.button_hot).bool()
        {
            let first = f64::from(self.prop(k, p.button_hot_first).float());
            let now = self.now;
            if let Some(b) = self.el_mut(k).and_then(|e| e.widget.button.as_mut()) {
                b.repeating = true;
                b.next = now + first;
            }
            self.want_frame(k);
            self.post(
                k,
                self.v.msgs.button_hot_click,
                MsgArg {
                    command: Some(cmd),
                    ..MsgArg::default()
                },
            );
        }
        self.button_choose_state(k);
    }

    fn button_release(&mut self, k: ElementKey) {
        let p = self.v.pids;
        let (over, repeating) = self
            .el(k)
            .and_then(|e| e.widget.button.as_ref())
            .map(|b| (b.over, b.repeating))
            .unwrap_or((false, false));
        let ghosted = self.prop(k, p.button_ghosted).bool();
        let mut click = false;
        if over && (!ghosted || self.prop(k, p.button_click_when_ghosted).bool()) {
            if self.prop(k, p.button_boolean).bool() {
                let h = self.prop(k, p.button_highlighted).bool();
                self.set_property(k, p.button_highlighted, PropertyValue::Bool(!h));
            }
            click = !repeating;
        }
        if let Some(b) = self.el_mut(k).and_then(|e| e.widget.button.as_mut()) {
            b.pressed_by = None;
            b.repeating = false;
        }
        self.button_choose_state(k);
        if click {
            self.post(
                k,
                self.v.msgs.button_click,
                MsgArg {
                    command: Some(Command::named(commands::SELECT_LEFT)),
                    ..MsgArg::default()
                },
            );
            let plugin_enabled = self
                .el(k)
                .and_then(|e| e.plugin.as_ref())
                .map(|pd| pd.enabled);
            if plugin_enabled == Some(true) {
                self.post(k, tessera_msg::CONTROL_CLICK, MsgArg::default());
            }
        }
    }

    /// Per-frame auto-repeat (`ui-widgets.md` §7.1 step 2). Returns whether it keeps running.
    pub(crate) fn button_frame(&mut self, k: ElementKey) -> bool {
        let Some(b) = self.el(k).and_then(|e| e.widget.button.clone()) else {
            return false;
        };
        if !b.repeating {
            return false;
        }
        let now = self.now;
        if b.over {
            if now >= b.next {
                let iv =
                    f64::from(self.prop(k, self.v.pids.button_hot_interval).float()).max(0.001);
                if let Some(bp) = self.el_mut(k).and_then(|e| e.widget.button.as_mut()) {
                    bp.next += iv;
                    if bp.next < now - 1.0 {
                        bp.next = now;
                    }
                }
                self.post(k, self.v.msgs.button_hot_click, MsgArg::value(0));
            }
        } else if now > b.next
            && let Some(bp) = self.el_mut(k).and_then(|e| e.widget.button.as_mut())
        {
            bp.next = now;
        }
        true
    }
}
