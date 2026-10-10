// SPDX-License-Identifier: GPL-3.0-or-later
//! The table between the original client's InputActions command names and
//! Tessera's (context, action) pairs (`keymap.md` §12.14.2), and the command
//! an event or a binding entry reports (§2.2, §12.15).
//!
//! The table holds **names** only. Command IDs are read at run time from
//! the user's install and handed to the mapper
//! ([`InputMapper::set_command_ids`](crate::InputMapper::set_command_ids));
//! code never hard-codes them (§2.2).

use crate::action::ActionKind;
use crate::defaults::contexts;
use crate::keymap::BindingSpec;

/// The value column of one row of §12.14.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowValue {
    /// `+1` (axis actions).
    Plus,
    /// `−1` (axis actions).
    Minus,
    /// "—": a button action.
    Button,
    /// "axis entry": an analog entry of an axis action (`look_x`, `look_y`).
    AxisEntry,
}

/// One row of §12.14.2, expanded: one command, one context, one action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandRow {
    /// The original's input map (section) the command is read from.
    pub section: &'static str,
    /// The InputActions command name (compared without regard to case).
    pub command: &'static str,
    /// Tessera context.
    pub context: &'static str,
    /// Tessera action.
    pub action: &'static str,
    /// The entry value or kind.
    pub value: RowValue,
}

const MAIN: &str = "Main";
const SCROLL_MOUSE: &str = "ScrollableControls_Mouse";
const DIALOG: &str = "DialogBoxes";
const EDIT: &str = "EditControls";
const CLIPBOARD: &str = "CopyAndPasteControls";

/// Compact form of the table: (section, commands, contexts, actions, value).
/// Several commands with several actions pair in order; several commands
/// with one action all map to it; every listed context gets the row (the
/// "camera **and** movement" row and the `ui.chat_log` copy rule).
type Compact = (
    &'static str,
    &'static [&'static str],
    &'static [&'static str],
    &'static [&'static str],
    RowValue,
);

const TABLE: &[Compact] = {
    use RowValue::{AxisEntry, Button, Minus, Plus};
    use contexts::{
        CAMERA, GLOBAL, MOVEMENT, POINTER, TEXT_ENTRY, UI_CHAT_LOG, UI_CLIPBOARD, UI_DIALOG,
        UI_SCROLL_MOUSE,
    };
    &[
        (
            MAIN,
            &[
                "MovementForward",
                "MovementForwardCameraMovement",
                "MovementForwardCameraMovementToggleMouseSteer",
            ],
            &[MOVEMENT],
            &["move_forward"],
            Plus,
        ),
        (
            MAIN,
            &["MovementBackup"],
            &[MOVEMENT],
            &["move_forward"],
            Minus,
        ),
        (
            MAIN,
            &["MovementStrafeRight"],
            &[MOVEMENT],
            &["strafe"],
            Plus,
        ),
        (
            MAIN,
            &["MovementStrafeLeft"],
            &[MOVEMENT],
            &["strafe"],
            Minus,
        ),
        (
            MAIN,
            &["MovementTurnLeft", "MovementTurnOrStrafeLeft"],
            &[MOVEMENT],
            &["turn"],
            Plus,
        ),
        (
            MAIN,
            &["MovementTurnRight", "MovementTurnOrStrafeRight"],
            &[MOVEMENT],
            &["turn"],
            Minus,
        ),
        (
            MAIN,
            &["MovementLongJump", "MovementHighJump"],
            &[MOVEMENT],
            &["jump"],
            Button,
        ),
        (
            MAIN,
            &["MovementRunLock"],
            &[MOVEMENT],
            &["autorun"],
            Button,
        ),
        (
            MAIN,
            &["CameraInstantMouseSteer"],
            &[CAMERA, MOVEMENT],
            &["mouselook"],
            Button,
        ),
        (MAIN, &["PointerX"], &[CAMERA], &["look_x"], AxisEntry),
        (MAIN, &["PointerY"], &[CAMERA], &["look_y"], AxisEntry),
        (
            MAIN,
            &["CameraInstantMoveToward", "CameraMoveToward"],
            &[CAMERA],
            &["zoom"],
            Plus,
        ),
        (
            MAIN,
            &["CameraInstantMoveAway", "CameraMoveAway"],
            &[CAMERA],
            &["zoom"],
            Minus,
        ),
        (MAIN, &["CameraReset"], &[CAMERA], &["reset_camera"], Button),
        (
            MAIN,
            &["CameraFlightMode"],
            &[GLOBAL],
            &["toggle_fly"],
            Button,
        ),
        (
            MAIN,
            &["CaptureScreenshot"],
            &[GLOBAL],
            &["screenshot"],
            Button,
        ),
        (
            MAIN,
            &["ToggleDebugHUD"],
            &[GLOBAL],
            &["toggle_debug"],
            Button,
        ),
        (MAIN, &["EscapeKey"], &[GLOBAL], &["menu"], Button),
        // P1 panels (`keymap.md` §12.14.2 OPEN: rows added by the client implementer).
        (
            MAIN,
            &["ToggleJournalPanel"],
            &[GLOBAL],
            &["toggle_character"],
            Button,
        ),
        (
            MAIN,
            &["QUICKSLOT_BACKPACK"],
            &[GLOBAL],
            &["toggle_bags"],
            Button,
        ),
        (
            MAIN,
            &[
                "QUICKSLOT_BACKPACK1",
                "QUICKSLOT_BACKPACK2",
                "QUICKSLOT_BACKPACK3",
                "QUICKSLOT_BACKPACK4",
                "QUICKSLOT_BACKPACK5",
                "QUICKSLOT_BACKPACK6",
            ],
            &[GLOBAL],
            &[
                "toggle_bag_1",
                "toggle_bag_2",
                "toggle_bag_3",
                "toggle_bag_4",
                "toggle_bag_5",
                "toggle_bag_6",
            ],
            Button,
        ),
        (
            MAIN,
            &["TOOLTIP_DETACH"],
            &[GLOBAL],
            &["pin_tooltip"],
            Button,
        ),
        (MAIN, &["AcceptInput"], &[GLOBAL], &["open_chat"], Button),
        (MAIN, &["SelectLeft"], &[POINTER], &["select_left"], Button),
        (
            MAIN,
            &["SelectRight"],
            &[POINTER],
            &["select_right"],
            Button,
        ),
        (MAIN, &["SelectMid"], &[POINTER], &["select_middle"], Button),
        (
            MAIN,
            &["SelectDblLeft"],
            &[POINTER],
            &["select_double_left"],
            Button,
        ),
        (
            MAIN,
            &["SelectDblRight"],
            &[POINTER],
            &["select_double_right"],
            Button,
        ),
        (
            MAIN,
            &["SelectDblMid"],
            &[POINTER],
            &["select_double_middle"],
            Button,
        ),
        (MAIN, &["TapLeft"], &[POINTER], &["tap_left"], Button),
        (MAIN, &["TapRight"], &[POINTER], &["tap_right"], Button),
        (MAIN, &["TapMid"], &[POINTER], &["tap_middle"], Button),
        (
            SCROLL_MOUSE,
            &["ScrollUp"],
            &[UI_SCROLL_MOUSE],
            &["scroll_up"],
            Button,
        ),
        (
            SCROLL_MOUSE,
            &["ScrollDown"],
            &[UI_SCROLL_MOUSE],
            &["scroll_down"],
            Button,
        ),
        (
            DIALOG,
            &["FocusNext"],
            &[UI_DIALOG],
            &["focus_next"],
            Button,
        ),
        (
            DIALOG,
            &["FocusPrevious"],
            &[UI_DIALOG],
            &["focus_prev"],
            Button,
        ),
        (
            DIALOG,
            &[
                "FocusMoveLeft",
                "FocusMoveRight",
                "FocusMoveUp",
                "FocusMoveDown",
            ],
            &[UI_DIALOG],
            &["focus_left", "focus_right", "focus_up", "focus_down"],
            Button,
        ),
        (DIALOG, &["EscapeKey"], &[UI_DIALOG], &["menu"], Button),
        (
            EDIT,
            &["AcceptInput"],
            &[TEXT_ENTRY],
            &["open_chat"],
            Button,
        ),
        (EDIT, &["EscapeKey"], &[TEXT_ENTRY], &["menu"], Button),
        (
            EDIT,
            &["BackspaceKey"],
            &[TEXT_ENTRY],
            &["text_backspace"],
            Button,
        ),
        (
            EDIT,
            &["DeleteKey"],
            &[TEXT_ENTRY],
            &["text_delete"],
            Button,
        ),
        (
            EDIT,
            &["CursorPreviousLine"],
            &[TEXT_ENTRY],
            &["text_line_up"],
            Button,
        ),
        (
            EDIT,
            &["CursorNextLine"],
            &[TEXT_ENTRY],
            &["text_line_down"],
            Button,
        ),
        (
            EDIT,
            &["CursorCharLeft", "CursorCharRight"],
            &[TEXT_ENTRY],
            &["text_char_left", "text_char_right"],
            Button,
        ),
        (
            EDIT,
            &["CursorWordLeft", "CursorWordRight"],
            &[TEXT_ENTRY],
            &["text_word_left", "text_word_right"],
            Button,
        ),
        (
            EDIT,
            &["CursorStartOfLine", "CursorEndOfLine"],
            &[TEXT_ENTRY],
            &["text_line_start", "text_line_end"],
            Button,
        ),
        (
            EDIT,
            &["CursorPreviousPage", "CursorNextPage"],
            &[TEXT_ENTRY, UI_CHAT_LOG],
            &["text_page_up", "text_page_down"],
            Button,
        ),
        (
            EDIT,
            &["CursorStartOfDocument", "CursorEndOfDocument"],
            &[TEXT_ENTRY, UI_CHAT_LOG],
            &["text_doc_start", "text_doc_end"],
            Button,
        ),
        (
            CLIPBOARD,
            &["CopyText", "CutText", "PasteText"],
            &[UI_CLIPBOARD],
            &["text_copy", "text_cut", "text_paste"],
            Button,
        ),
    ]
};

/// Every row of §12.14.2, expanded to one (command, context, action) each,
/// in table order (the order §12.15 picks from).
#[must_use]
pub fn command_rows() -> Vec<CommandRow> {
    let mut out = Vec::new();
    for &(section, commands, ctxs, actions, value) in TABLE {
        for (i, &command) in commands.iter().enumerate() {
            let action = if actions.len() == commands.len() {
                actions[i]
            } else {
                actions[0]
            };
            for &context in ctxs {
                out.push(CommandRow {
                    section,
                    command,
                    context,
                    action,
                    value,
                });
            }
        }
    }
    out
}

/// The command §12.15 reports for one entry of `(context, action)` that
/// remembers no command (§2.2 rule 2): button actions take the first row's
/// command; digital entries of axis actions the first row with the sign of
/// their value; analog entries the first "axis entry" row, else the first
/// row with the sign of their scale. `None` for pairs without rows, a value
/// or scale of exactly 0, and digital entries of `look_x`/`look_y`.
#[must_use]
pub fn command_for_entry(
    context: &str,
    action: &str,
    kind: ActionKind,
    entry: &BindingSpec,
) -> Option<&'static str> {
    let rows: Vec<CommandRow> = command_rows()
        .into_iter()
        .filter(|r| r.context == context && r.action == action)
        .collect();
    let by_sign = |v: f32| -> Option<&'static str> {
        let want = if v > 0.0 {
            RowValue::Plus
        } else if v < 0.0 {
            RowValue::Minus
        } else {
            return None;
        };
        rows.iter().find(|r| r.value == want).map(|r| r.command)
    };
    match (kind, entry) {
        (ActionKind::Button, _) => rows
            .iter()
            .find(|r| r.value == RowValue::Button)
            .map(|r| r.command),
        (ActionKind::Axis, BindingSpec::Digital { value, .. }) => by_sign(*value),
        (ActionKind::Axis, BindingSpec::Analog { scale, .. }) => rows
            .iter()
            .find(|r| r.value == RowValue::AxisEntry)
            .map(|r| r.command)
            .or_else(|| by_sign(*scale)),
    }
}

/// Built-in contexts per original input map (`keymap.md` §8.1): the
/// context `tessera-ui`'s push and pop requests by InputMap name map to.
/// Names compare without regard to ASCII case. `Main` and the ignored maps
/// give `None`; the chat window's Tessera name `ui.chat_log` maps to itself.
#[must_use]
pub fn context_for_input_map(name: &str) -> Option<&'static str> {
    const MAP: [(&str, &str); 7] = [
        ("DialogBoxes", contexts::UI_DIALOG),
        ("EditControls", contexts::TEXT_ENTRY),
        ("CopyAndPasteControls", contexts::UI_CLIPBOARD),
        ("ScrollableControls_Mouse", contexts::UI_SCROLL_MOUSE),
        ("ScrollableControls", contexts::UI_SCROLL),
        ("EatKeyboardInput", contexts::UI_EAT_KEYBOARD),
        (contexts::UI_CHAT_LOG, contexts::UI_CHAT_LOG),
    ];
    MAP.iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|&(_, c)| c)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chord::Chord;
    use crate::device::AnalogAxis;

    #[test]
    fn rows_expand_in_order() {
        let rows = command_rows();
        let ms: Vec<_> = rows
            .iter()
            .filter(|r| r.command == "CameraInstantMouseSteer")
            .map(|r| r.context)
            .collect();
        assert_eq!(ms, [contexts::CAMERA, contexts::MOVEMENT]);
        let focus: Vec<_> = rows
            .iter()
            .filter(|r| r.command.starts_with("FocusMove"))
            .map(|r| (r.command, r.action))
            .collect();
        assert_eq!(
            focus,
            [
                ("FocusMoveLeft", "focus_left"),
                ("FocusMoveRight", "focus_right"),
                ("FocusMoveUp", "focus_up"),
                ("FocusMoveDown", "focus_down")
            ]
        );
        let page: Vec<_> = rows
            .iter()
            .filter(|r| r.command == "CursorNextPage")
            .map(|r| (r.context, r.action))
            .collect();
        assert_eq!(
            page,
            [
                (contexts::TEXT_ENTRY, "text_page_down"),
                (contexts::UI_CHAT_LOG, "text_page_down")
            ]
        );
    }

    #[test]
    fn p1_panel_commands() {
        use ActionKind::Button;
        let d = |t: &str| BindingSpec::digital(Chord::parse(t).unwrap(), 1.0);
        assert_eq!(
            command_for_entry("global", "toggle_character", Button, &d("C")),
            Some("ToggleJournalPanel")
        );
        assert_eq!(
            command_for_entry("global", "toggle_bags", Button, &d("B")),
            Some("QUICKSLOT_BACKPACK")
        );
        for n in 1..=6 {
            let action = format!("toggle_bag_{n}");
            let want = format!("QUICKSLOT_BACKPACK{n}");
            assert_eq!(
                command_for_entry("global", &action, Button, &d("I")),
                Some(want.as_str())
            );
        }
        assert_eq!(
            command_for_entry("global", "pin_tooltip", Button, &d("Ctrl+T")),
            Some("TOOLTIP_DETACH")
        );
    }

    #[test]
    fn section_12_15_examples() {
        use ActionKind::{Axis, Button};
        let d = |t: &str, v: f32| BindingSpec::digital(Chord::parse(t).unwrap(), v);
        let a = |axis, s: f32| BindingSpec::analog(axis, s, 0.0);
        assert_eq!(
            command_for_entry("movement", "jump", Button, &d("Space", 1.0)),
            Some("MovementLongJump")
        );
        for c in ["camera", "movement"] {
            assert_eq!(
                command_for_entry(c, "mouselook", Button, &d("MouseRight", 1.0)),
                Some("CameraInstantMouseSteer")
            );
        }
        for c in ["global", "text_entry"] {
            assert_eq!(
                command_for_entry(c, "open_chat", Button, &d("Enter", 1.0)),
                Some("AcceptInput")
            );
        }
        for c in ["global", "ui.dialog", "text_entry"] {
            assert_eq!(
                command_for_entry(c, "menu", Button, &d("Escape", 1.0)),
                Some("EscapeKey")
            );
        }
        assert_eq!(
            command_for_entry("ui.chat_log", "text_page_up", Button, &d("PageUp", 1.0)),
            Some("CursorPreviousPage")
        );
        assert_eq!(
            command_for_entry("movement", "move_forward", Axis, &d("S", -1.0)),
            Some("MovementBackup")
        );
        assert_eq!(
            command_for_entry("movement", "move_forward", Axis, &d("S", 0.0)),
            None
        );
        assert_eq!(
            command_for_entry("camera", "look_x", Axis, &d("A", 1.0)),
            None
        );
        assert_eq!(
            command_for_entry("camera", "look_x", Axis, &a(AnalogAxis::MouseX, -3.0)),
            Some("PointerX")
        );
        assert_eq!(
            command_for_entry("camera", "zoom", Axis, &a(AnalogAxis::WheelY, 1.0)),
            Some("CameraInstantMoveToward")
        );
        assert_eq!(
            command_for_entry("camera", "zoom", Axis, &a(AnalogAxis::WheelY, 0.0)),
            None
        );
        assert_eq!(
            command_for_entry(
                "movement",
                "move_forward",
                Axis,
                &a(AnalogAxis::PadLeftY, 1.0)
            ),
            Some("MovementForward")
        );
        assert_eq!(
            command_for_entry("fly", "fly_vertical", Axis, &d("Space", 1.0)),
            None
        );
    }

    #[test]
    fn input_maps() {
        assert_eq!(context_for_input_map("editcontrols"), Some("text_entry"));
        assert_eq!(context_for_input_map("ui.chat_log"), Some("ui.chat_log"));
        assert_eq!(context_for_input_map("Main"), None);
        assert_eq!(context_for_input_map("VendorBuyUI"), None);
    }
}
