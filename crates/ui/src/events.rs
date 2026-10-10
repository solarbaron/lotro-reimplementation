// SPDX-License-Identifier: GPL-3.0-or-later
//! UI messages (`ui-behaviour.md` §12) and the events the UI hands to the client and the
//! plugin host.
//!
//! **Delivery.** [`crate::Ui::post`] numbers a message and delivers it at once when no
//! delivery is running, otherwise appends it to one FIFO queue that the outermost delivery
//! drains before it returns (§12.1 step 1). Native handlers (the widget classes) and
//! attached listeners run inside the UI. What leaves the UI goes to an outbox of
//! [`UiEvent`]s, in delivery order:
//!
//! - **plugin notifications** ([`UiEvent::Control`]): the plugin control's listener turns a
//!   notification into a Lua event only when its control is the source
//!   (`lua-plugin-api.md` §9.7). The plugin host drains the outbox after each UI call it
//!   makes and after each offer; a host call made while it drains only appends, so events
//!   caused by a handler's calls arrive after the current event, in call order, as §9.7
//!   requires;
//! - **game-code handlers** ([`UiEvent::Game`]): the listener registry's handlers for
//!   (source element ID, message), called at the top of the walk unless the message was
//!   stopped (§12.1 step 3 and its Tessera rule);
//! - **broadcasts and client work**: the key broadcast of unconsumed key commands
//!   (`lua-plugin-api.md` §13.13 step 3), sounds, layout-changed notices, …
//!
//! Tessera difference (recorded): a plugin listener of the original runs during the
//! delivery of its notification; here its Lua event runs after the UI's own handling of the
//! notifications queued meanwhile. Plugin listeners never stop or handle a notification
//! (§9.7), so only a handler that reads state another queued notification changes can tell.

use std::sync::Arc;

use crate::tree::ElementKey;

/// An input command carried by a message (`ui-behaviour.md` §3.1).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Command {
    /// The InputActions command name.
    pub name: Arc<str>,
    /// The InputActions ID, when the client resolved it.
    pub id: Option<u32>,
}

impl Command {
    /// A command by name, without an ID.
    #[must_use]
    pub fn named(name: &str) -> Self {
        Self {
            name: Arc::from(name),
            id: None,
        }
    }

    /// Whether it is one of the nine mouse-button commands.
    #[must_use]
    pub fn is_mouse_button(&self) -> bool {
        crate::vocab::commands::MOUSE_BUTTONS.contains(&&*self.name)
    }

    /// Whether it is a wheel command (`ScrollUp`, `ScrollDown`, `Scroll`).
    #[must_use]
    pub fn is_wheel(&self) -> bool {
        use crate::vocab::commands::{SCROLL, SCROLL_DOWN, SCROLL_UP};
        matches!(&*self.name, SCROLL_UP | SCROLL_DOWN | SCROLL)
    }

    /// The plugin `MouseButton` of the command (`lua-plugin-api.md` §13.12.1): 1 left,
    /// 2 right, 4 middle, 0 otherwise (by ID when known, else by name).
    #[must_use]
    pub fn mouse_button(&self) -> u32 {
        if let Some(id) = self.id {
            return match id {
                0x32 | 0x33 | 0x56 | 0x66 | 0x72 => 1,
                0x03 | 0x2E | 0x43 | 0x4C | 0x73 => 2,
                0x67 | 0x6E | 0x93 | 0xA4 => 4,
                _ => 0,
            };
        }
        match &*self.name {
            "SelectLeft" | "SelectDblLeft" | "TapLeft" => 1,
            "SelectRight" | "SelectDblRight" | "TapRight" => 2,
            "SelectMid" | "SelectDblMid" | "TapMid" => 4,
            _ => 0,
        }
    }

    /// The single-click form of a double-click command (`ui-behaviour.md` §4.6).
    #[must_use]
    pub fn single_click(&self) -> Self {
        let (name, id) = match &*self.name {
            "SelectDblLeft" => ("SelectLeft", 0x56),
            "SelectDblRight" => ("SelectRight", 0x4C),
            "SelectDblMid" => ("SelectMid", 0xA4),
            _ => return self.clone(),
        };
        Self {
            name: Arc::from(name),
            id: self.id.map(|_| id),
        }
    }

    /// Whether it is one of the three double-click commands.
    #[must_use]
    pub fn is_double(&self) -> bool {
        matches!(
            &*self.name,
            "SelectDblLeft" | "SelectDblRight" | "SelectDblMid"
        )
    }
}

/// The argument of a message.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MsgArg {
    /// The command (presses, releases, clicks, keys, wheel).
    pub command: Option<Command>,
    /// A position relative to the receiving element (presses, moves, hover, wheel).
    pub pos: (i32, i32),
    /// A value (enter/leave 1/0, focus 1/0, key value × 1000, selection index, …).
    pub value: i64,
    /// A second value (selection: the entry's element key owner; stop index).
    pub value2: i64,
}

impl MsgArg {
    /// An argument holding only a value.
    #[must_use]
    pub fn value(v: i64) -> Self {
        Self {
            value: v,
            ..Self::default()
        }
    }
}

/// A posted message (`ui-behaviour.md` §12.1).
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    /// Global sequence number.
    pub seq: u64,
    /// The source element.
    pub source: ElementKey,
    /// The source's element ID.
    pub source_id: u32,
    /// The message value (`UIElementMessage`, or a [`crate::vocab::tessera_msg`] value).
    pub msg: u32,
    /// The argument.
    pub arg: MsgArg,
}

/// The answer of a handler (`ui-behaviour.md` §12.1 step 2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Answer {
    /// Not handled.
    #[default]
    NotHandled,
    /// Handled; the walk goes on.
    Handled,
    /// Stop: the delivery ends at once.
    Stop,
}

/// A plugin-control event (`lua-plugin-api.md` §9, §13): the plugin-side view of a
/// notification whose source is the control.
#[derive(Debug, Clone, PartialEq)]
pub enum ControlEvent {
    /// `MouseEnter`.
    MouseEnter,
    /// `MouseLeave`.
    MouseLeave,
    /// `MouseDown`.
    MouseDown {
        /// `MouseButton`.
        button: u32,
        /// Relative position.
        x: i32,
        /// Relative position.
        y: i32,
    },
    /// `MouseUp`.
    MouseUp {
        /// `MouseButton`.
        button: u32,
        /// Relative position.
        x: i32,
        /// Relative position.
        y: i32,
    },
    /// `MouseClick`.
    MouseClick {
        /// `MouseButton`.
        button: u32,
        /// Relative position.
        x: i32,
        /// Relative position.
        y: i32,
    },
    /// `MouseDoubleClick`.
    MouseDoubleClick {
        /// `MouseButton`.
        button: u32,
        /// Relative position.
        x: i32,
        /// Relative position.
        y: i32,
    },
    /// `MouseMove`.
    MouseMove {
        /// Relative position.
        x: i32,
        /// Relative position.
        y: i32,
    },
    /// `MouseHover`.
    MouseHover {
        /// Relative position.
        x: i32,
        /// Relative position.
        y: i32,
    },
    /// `MouseWheel`.
    MouseWheel {
        /// +1 up, −1 down.
        direction: i32,
        /// Relative position.
        x: i32,
        /// Relative position.
        y: i32,
    },
    /// `KeyDown` from a focused text box's own notification (§13.13 step 2).
    KeyDown {
        /// The command.
        command: Command,
    },
    /// `KeyUp` from a focused text box's own notification.
    KeyUp {
        /// The command.
        command: Command,
    },
    /// `FocusGained`.
    FocusGained,
    /// `FocusLost`.
    FocusLost,
    /// `SizeChanged`.
    SizeChanged,
    /// `PositionChanged`.
    PositionChanged,
    /// `VisibleChanged`.
    VisibleChanged,
    /// `Activated`.
    Activated,
    /// `Deactivated`.
    Deactivated,
    /// `Button.Click`.
    Click,
    /// `TextBox.TextChanged`.
    TextChanged,
    /// `DragStart`.
    DragStart,
    /// `DragEnter`.
    DragEnter,
    /// `DragLeave`.
    DragLeave,
    /// `DragDrop`.
    DragDrop,
}

/// Something the UI tells the client or the plugin host.
#[derive(Debug, Clone, PartialEq)]
pub enum UiEvent {
    /// A plugin-control event.
    Control {
        /// The control.
        control: ElementKey,
        /// The event.
        event: ControlEvent,
    },
    /// The listener registry's handlers for (source element ID, message): call them in
    /// registration order (`ui-behaviour.md` §12.1 step 3).
    Game {
        /// The message.
        message: Message,
    },
    /// A key command that no element consumed: the plugins' `KeyDown`/`KeyUp` broadcast and
    /// game code (`ui-behaviour.md` §5.3 step 3, `lua-plugin-api.md` §13.13 step 3).
    KeyBroadcast {
        /// Press (true) or release.
        down: bool,
        /// The command.
        command: Command,
    },
    /// Play a UI sound (`ui-layout.md` §6.9).
    Sound {
        /// The sound-info DID.
        did: u32,
    },
    /// Play a movie (`ui-layout.md` §6.9; Tessera M5 plays none).
    Movie {
        /// The element.
        element: ElementKey,
        /// Movie DID.
        did: u32,
        /// File path.
        path: String,
    },
    /// The UI manager's "layout changed" notification after a move of a `save_loc` element
    /// or a resize of a `save_size` element (`ui-behaviour.md` §8.4 step 2).
    LayoutChanged {
        /// The element.
        element: ElementKey,
    },
    /// The screen was resized (`ui-behaviour.md` §9.4 step 3).
    ScreenResized {
        /// New width.
        width: i32,
        /// New height.
        height: i32,
    },
    /// The cursor chosen changed (`ui-behaviour.md` §4.8): an image DID and hot spot, or
    /// `None` for the default cursor.
    Cursor {
        /// Image and hot spot.
        cursor: Option<(u32, u32, u32)>,
    },
    /// An item drag began (a drag icon exists; decision 0014 D7).
    DragBegan {
        /// The source.
        source: ElementKey,
    },
    /// An item drag ended (`ui-behaviour.md` §7.4).
    DragEnded {
        /// The source.
        source: ElementKey,
        /// The drop target, if any.
        target: Option<ElementKey>,
        /// Whether the drop was accepted.
        accepted: bool,
    },
    /// A text element wants the clipboard (`ui-behaviour.md` §5.4): copy this text.
    CopyText {
        /// The text.
        text: String,
    },
    /// A focused editable element got `PasteText` (`ui-behaviour.md` §5.4): the client
    /// answers with [`crate::Ui::paste_text`] and the clipboard text.
    PasteRequest {
        /// The element to paste into.
        target: ElementKey,
    },
    /// An engine dialog closed with the result of its yes or no button
    /// (`ui-behaviour.md` §5.5).
    DialogResult {
        /// The dialog.
        dialog: ElementKey,
        /// Yes (`true`) or no.
        yes: bool,
    },
    /// The hover handler ran for an element that asks for an examination tooltip
    /// ([`crate::Ui::set_examine`]; `item-tooltips.md` §9): the client builds the item's
    /// tooltip and shows it with [`crate::game::examine::ExamTooltips::show`].
    ExamineHover {
        /// The element that asked (the hovered element or its nearest ancestor that asks).
        element: ElementKey,
        /// The pointer position on the screen.
        pointer: (i32, i32),
    },
    /// A UI change request could not be applied.
    Warning {
        /// What happened.
        what: String,
    },
}
