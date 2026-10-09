// SPDX-License-Identifier: GPL-3.0-or-later
//! Control names and codes of the original's key-mapping file
//! (`keymap.md` §12.6): DirectInput key, mouse and joystick offsets (public
//! header constants) and their Tessera counterparts.

use crate::device::{KeyCode, MouseButton, PadButton};

/// Device type codes (§12.5.2).
pub mod device_type {
    /// `Keyboard`.
    pub const KEYBOARD: u8 = 1;
    /// `Mouse`.
    pub const MOUSE: u8 = 2;
    /// `Joystick`.
    pub const JOYSTICK: u8 = 3;
    /// `Virtual`.
    pub const VIRTUAL: u8 = 4;
}

/// The device type words, matched without regard to ASCII case (§12.5.2).
pub const DEVICE_TYPES: [(&str, u8); 4] = [
    ("Keyboard", device_type::KEYBOARD),
    ("Mouse", device_type::MOUSE),
    ("Joystick", device_type::JOYSTICK),
    ("Virtual", device_type::VIRTUAL),
];

/// The device type word written for a type code.
#[must_use]
pub fn device_type_word(code: u8) -> Option<&'static str> {
    DEVICE_TYPES
        .iter()
        .find(|(_, c)| *c == code)
        .map(|(w, _)| *w)
}

/// A device type code from its word (ASCII case-insensitive).
#[must_use]
pub fn device_type_from_word(word: &[u8]) -> Option<u8> {
    DEVICE_TYPES
        .iter()
        .find(|(w, _)| w.as_bytes().eq_ignore_ascii_case(word))
        .map(|(_, c)| *c)
}

/// Keyboard names (§12.6.1): (written name, code, Tessera key name).
const KEYBOARD: &[(&str, u16, Option<&str>)] = &[
    ("DIK_ESCAPE", 0x01, Some("Escape")),
    ("DIK_1", 0x02, Some("1")),
    ("DIK_2", 0x03, Some("2")),
    ("DIK_3", 0x04, Some("3")),
    ("DIK_4", 0x05, Some("4")),
    ("DIK_5", 0x06, Some("5")),
    ("DIK_6", 0x07, Some("6")),
    ("DIK_7", 0x08, Some("7")),
    ("DIK_8", 0x09, Some("8")),
    ("DIK_9", 0x0A, Some("9")),
    ("DIK_0", 0x0B, Some("0")),
    ("DIK_MINUS", 0x0C, Some("Minus")),
    ("DIK_EQUALS", 0x0D, Some("Equal")),
    ("DIK_BACK", 0x0E, Some("Backspace")),
    ("DIK_TAB", 0x0F, Some("Tab")),
    ("DIK_Q", 0x10, Some("Q")),
    ("DIK_W", 0x11, Some("W")),
    ("DIK_E", 0x12, Some("E")),
    ("DIK_R", 0x13, Some("R")),
    ("DIK_T", 0x14, Some("T")),
    ("DIK_Y", 0x15, Some("Y")),
    ("DIK_U", 0x16, Some("U")),
    ("DIK_I", 0x17, Some("I")),
    ("DIK_O", 0x18, Some("O")),
    ("DIK_P", 0x19, Some("P")),
    ("DIK_LBRACKET", 0x1A, Some("BracketLeft")),
    ("DIK_RBRACKET", 0x1B, Some("BracketRight")),
    ("DIK_RETURN", 0x1C, Some("Enter")),
    ("DIK_LCONTROL", 0x1D, Some("LCtrl")),
    ("DIK_A", 0x1E, Some("A")),
    ("DIK_S", 0x1F, Some("S")),
    ("DIK_D", 0x20, Some("D")),
    ("DIK_F", 0x21, Some("F")),
    ("DIK_G", 0x22, Some("G")),
    ("DIK_H", 0x23, Some("H")),
    ("DIK_J", 0x24, Some("J")),
    ("DIK_K", 0x25, Some("K")),
    ("DIK_L", 0x26, Some("L")),
    ("DIK_SEMICOLON", 0x27, Some("Semicolon")),
    ("DIK_APOSTROPHE", 0x28, Some("Quote")),
    ("DIK_GRAVE", 0x29, Some("Backquote")),
    ("DIK_LSHIFT", 0x2A, Some("LShift")),
    ("DIK_BACKSLASH", 0x2B, Some("Backslash")),
    ("DIK_Z", 0x2C, Some("Z")),
    ("DIK_X", 0x2D, Some("X")),
    ("DIK_C", 0x2E, Some("C")),
    ("DIK_V", 0x2F, Some("V")),
    ("DIK_B", 0x30, Some("B")),
    ("DIK_N", 0x31, Some("N")),
    ("DIK_M", 0x32, Some("M")),
    ("DIK_COMMA", 0x33, Some("Comma")),
    ("DIK_PERIOD", 0x34, Some("Period")),
    ("DIK_SLASH", 0x35, Some("Slash")),
    ("DIK_RSHIFT", 0x36, Some("RShift")),
    ("DIK_NUMPADSTAR", 0x37, Some("NumpadMultiply")),
    ("DIK_LALT", 0x38, Some("LAlt")),
    ("DIK_SPACE", 0x39, Some("Space")),
    ("DIK_CAPSLOCK", 0x3A, Some("CapsLock")),
    ("DIK_F1", 0x3B, Some("F1")),
    ("DIK_F2", 0x3C, Some("F2")),
    ("DIK_F3", 0x3D, Some("F3")),
    ("DIK_F4", 0x3E, Some("F4")),
    ("DIK_F5", 0x3F, Some("F5")),
    ("DIK_F6", 0x40, Some("F6")),
    ("DIK_F7", 0x41, Some("F7")),
    ("DIK_F8", 0x42, Some("F8")),
    ("DIK_F9", 0x43, Some("F9")),
    ("DIK_F10", 0x44, Some("F10")),
    ("DIK_NUMLOCK", 0x45, Some("NumLock")),
    ("DIK_SCROLL", 0x46, Some("ScrollLock")),
    ("DIK_NUMPAD7", 0x47, Some("Numpad7")),
    ("DIK_NUMPAD8", 0x48, Some("Numpad8")),
    ("DIK_NUMPAD9", 0x49, Some("Numpad9")),
    ("DIK_NUMPADMINUS", 0x4A, Some("NumpadSubtract")),
    ("DIK_NUMPAD4", 0x4B, Some("Numpad4")),
    ("DIK_NUMPAD5", 0x4C, Some("Numpad5")),
    ("DIK_NUMPAD6", 0x4D, Some("Numpad6")),
    ("DIK_NUMPADPLUS", 0x4E, Some("NumpadAdd")),
    ("DIK_NUMPAD1", 0x4F, Some("Numpad1")),
    ("DIK_NUMPAD2", 0x50, Some("Numpad2")),
    ("DIK_NUMPAD3", 0x51, Some("Numpad3")),
    ("DIK_NUMPAD0", 0x52, Some("Numpad0")),
    ("DIK_NUMPADPERIOD", 0x53, Some("NumpadDecimal")),
    ("DIK_OEM_102", 0x56, Some("IntlBackslash")),
    ("DIK_F11", 0x57, Some("F11")),
    ("DIK_F12", 0x58, Some("F12")),
    ("DIK_F13", 0x64, Some("F13")),
    ("DIK_F14", 0x65, Some("F14")),
    ("DIK_F15", 0x66, Some("F15")),
    ("DIK_NUMPADENTER", 0x9C, Some("NumpadEnter")),
    ("DIK_RCONTROL", 0x9D, Some("RCtrl")),
    ("DIK_NUMPADSLASH", 0xB5, Some("NumpadDivide")),
    ("DIK_SYSRQ", 0xB7, Some("PrintScreen")),
    ("DIK_RALT", 0xB8, Some("RAlt")),
    ("DIK_PAUSE", 0xC5, Some("Pause")),
    ("DIK_HOME", 0xC7, Some("Home")),
    ("DIK_END", 0xCF, Some("End")),
    ("DIK_UPARROW", 0xC8, Some("Up")),
    ("DIK_DOWNARROW", 0xD0, Some("Down")),
    ("DIK_LEFTARROW", 0xCB, Some("Left")),
    ("DIK_RIGHTARROW", 0xCD, Some("Right")),
    ("DIK_PGUP", 0xC9, Some("PageUp")),
    ("DIK_PGDN", 0xD1, Some("PageDown")),
    ("DIK_INSERT", 0xD2, Some("Insert")),
    ("DIK_DELETE", 0xD3, Some("Delete")),
    ("DIK_LWIN", 0xDB, Some("LSuper")),
    ("DIK_RWIN", 0xDC, Some("RSuper")),
    ("DIK_APPS", 0xDD, Some("ContextMenu")),
    ("DIK_ABNT_C1", 0x73, None),
    ("DIK_YEN", 0x7D, None),
    ("DIK_ABNT_C2", 0x7E, None),
    ("DIK_NUMPADEQUALS", 0x8D, None),
    ("DIK_PREVTRACK", 0x90, None),
    ("DIK_AT", 0x91, None),
    ("DIK_COLON", 0x92, None),
    ("DIK_UNDERLINE", 0x93, None),
    ("DIK_STOP", 0x95, None),
    ("DIK_AX", 0x96, None),
    ("DIK_UNLABELED", 0x97, None),
    ("DIK_NEXTTRACK", 0x99, None),
    ("DIK_MUTE", 0xA0, None),
    ("DIK_CALCULATOR", 0xA1, None),
    ("DIK_PLAYPAUSE", 0xA2, None),
    ("DIK_MEDIASTOP", 0xA4, None),
    ("DIK_VOLUMEDOWN", 0xAE, None),
    ("DIK_VOLUMEUP", 0xB0, None),
    ("DIK_WEBHOME", 0xB2, None),
    ("DIK_NUMPADCOMMA", 0xB3, None),
    ("DIK_WEBSEARCH", 0xE5, None),
    ("DIK_WEBFAVORITES", 0xE6, None),
    ("DIK_WEBREFRESH", 0xE7, None),
    ("DIK_WEBSTOP", 0xE8, None),
    ("DIK_WEBFORWARD", 0xE9, None),
    ("DIK_WEBBACK", 0xEA, None),
    ("DIK_MYCOMPUTER", 0xEB, None),
    ("DIK_MAIL", 0xEC, None),
    ("DIK_MEDIASELECT", 0xED, None),
];

/// Keyboard aliases: accepted, never written (§12.6.1).
const KEYBOARD_ALIASES: &[(&str, u16)] = &[
    ("DIK_BACKSPACE", 0x0E),
    ("DIK_MULTIPLY", 0x37),
    ("DIK_LMENU", 0x38),
    ("DIK_CAPITAL", 0x3A),
    ("DIK_SUBTRACT", 0x4A),
    ("DIK_ADD", 0x4E),
    ("DIK_DECIMAL", 0x53),
    ("DIK_DIVIDE", 0xB5),
    ("DIK_RMENU", 0xB8),
    ("DIK_UP", 0xC8),
    ("DIK_DOWN", 0xD0),
    ("DIK_LEFT", 0xCB),
    ("DIK_RIGHT", 0xCD),
    ("DIK_PRIOR", 0xC9),
    ("DIK_NEXT", 0xD1),
];

/// Mouse names (§12.6.2), first spelling written.
const MOUSE: &[(&str, u16)] = &[
    ("DIMOFS_X", 0x00),
    ("DIMOFS_Y", 0x04),
    ("DIMOFS_Z", 0x08),
    ("DIMOFS_WHEEL", 0x08),
    ("DIMOFS_BUTTON0", 0x0C),
    ("DIMOFS_BUTTON1", 0x0D),
    ("DIMOFS_BUTTON2", 0x0E),
    ("DIMOFS_BUTTON3", 0x0F),
    ("DIMOFS_BUTTON4", 0x10),
    ("DIMOFS_BUTTON5", 0x11),
    ("DIMOFS_BUTTON6", 0x12),
    ("DIMOFS_BUTTON7", 0x13),
];

/// Mouse codes (§12.6.2).
pub mod mouse_code {
    /// Relative X axis.
    pub const X: u16 = 0x00;
    /// Relative Y axis.
    pub const Y: u16 = 0x04;
    /// The wheel.
    pub const WHEEL: u16 = 0x08;
    /// First button (`DIMOFS_BUTTON0`); buttons 0–7 follow.
    pub const BUTTON0: u16 = 0x0C;
}

/// Joystick codes (§12.6.3).
pub mod joy_code {
    /// Left stick X.
    pub const X: u16 = 0x00;
    /// Left stick Y.
    pub const Y: u16 = 0x04;
    /// Left trigger on an XInput slot.
    pub const Z: u16 = 0x08;
    /// Right stick X.
    pub const RX: u16 = 0x0C;
    /// Right stick Y.
    pub const RY: u16 = 0x10;
    /// Right trigger on an XInput slot.
    pub const RZ: u16 = 0x14;
    /// First hat (the D-pad on an XInput slot).
    pub const POV0: u16 = 0x20;
    /// First button; buttons 0–31 follow.
    pub const BUTTON0: u16 = 0x30;
    /// The left stick as one control.
    pub const STICK0: u16 = 0x400;
    /// The right stick as one control.
    pub const STICK1: u16 = 0x404;
}

/// The one virtual-device name (§12.6.4).
const VIRTUAL: &[(&str, u16)] = &[("DIV_MOUSELOOK", 0x01)];

/// Sub-control words and values (§12.5.8), matched without regard to ASCII
/// case.
pub const SUB_CONTROLS: [(&str, u8); 7] = [
    ("None", 0),
    ("AxisPositive", 1),
    ("AxisNegative", 2),
    ("POVUp", 3),
    ("POVRight", 4),
    ("POVDown", 5),
    ("POVLeft", 6),
];

/// A sub-control value from its word.
#[must_use]
pub fn sub_control_from_word(word: &[u8]) -> Option<u8> {
    SUB_CONTROLS
        .iter()
        .find(|(w, _)| w.as_bytes().eq_ignore_ascii_case(word))
        .map(|(_, v)| *v)
}

/// The word written for a sub-control value.
#[must_use]
pub fn sub_control_word(value: u8) -> Option<&'static str> {
    SUB_CONTROLS
        .iter()
        .find(|(_, v)| *v == value)
        .map(|(w, _)| *w)
}

/// Parse `PREFIXn` or `PREFIX(n)` with `n` in `0..count`, decimal without
/// leading zeros.
fn indexed(name: &[u8], prefix: &[u8], count: u16) -> Option<u16> {
    let rest = name.strip_prefix(prefix)?;
    let digits = rest
        .strip_prefix(b"(")
        .and_then(|r| r.strip_suffix(b")"))
        .unwrap_or(rest);
    if digits.is_empty()
        || digits.len() > 2
        || !digits.iter().all(u8::is_ascii_digit)
        || (digits.len() > 1 && digits[0] == b'0')
    {
        return None;
    }
    let n = digits
        .iter()
        .fold(0u16, |a, d| a * 10 + u16::from(d - b'0'));
    (n < count).then_some(n)
}

fn joystick_code(name: &[u8]) -> Option<u16> {
    let fixed: [(&[u8], u16); 8] = [
        (b"DIJOFS_X", joy_code::X),
        (b"DIJOFS_Y", joy_code::Y),
        (b"DIJOFS_Z", joy_code::Z),
        (b"DIJOFS_RX", joy_code::RX),
        (b"DIJOFS_RY", joy_code::RY),
        (b"DIJOFS_RZ", joy_code::RZ),
        (b"DIJOFS_STICK0", joy_code::STICK0),
        (b"DIJOFS_STICK1", joy_code::STICK1),
    ];
    if let Some(&(_, c)) = fixed.iter().find(|(n, _)| *n == name) {
        return Some(c);
    }
    if let Some(n) = indexed(name, b"DIJOFS_SLIDER", 2) {
        return Some(0x18 + 4 * n);
    }
    if let Some(n) = indexed(name, b"DIJOFS_POV", 4) {
        return Some(joy_code::POV0 + 4 * n);
    }
    indexed(name, b"DIJOFS_BUTTON", 32).map(|n| joy_code::BUTTON0 + n)
}

/// A control code from its name, looked up **with** regard to case in all
/// four name tables, whatever the device's type (§12.5.6).
#[must_use]
pub fn control_code(name: &[u8]) -> Option<u16> {
    KEYBOARD
        .iter()
        .map(|&(n, c, _)| (n, c))
        .chain(KEYBOARD_ALIASES.iter().copied())
        .chain(MOUSE.iter().copied())
        .chain(VIRTUAL.iter().copied())
        .find(|(n, _)| n.as_bytes() == name)
        .map(|(_, c)| c)
        .or_else(|| joystick_code(name))
}

/// The name the original writes for `code` on a device of type `ty`, looked
/// up only in that type's table (§12.13); `None` when it has none (written
/// as the empty token).
#[must_use]
pub fn control_name(ty: u8, code: u16) -> Option<String> {
    match ty {
        device_type::KEYBOARD => KEYBOARD
            .iter()
            .find(|&&(_, c, _)| c == code)
            .map(|(n, _, _)| (*n).to_owned()),
        device_type::MOUSE => MOUSE
            .iter()
            .find(|&&(_, c)| c == code)
            .map(|(n, _)| (*n).to_owned()),
        device_type::JOYSTICK => joystick_name(code),
        _ => None,
    }
}

/// The joystick spelling the original writes (§12.6.3 table).
fn joystick_name(code: u16) -> Option<String> {
    Some(match code {
        joy_code::X => "DIJOFS_X".into(),
        joy_code::Y => "DIJOFS_Y".into(),
        joy_code::Z => "DIJOFS_Z".into(),
        joy_code::RX => "DIJOFS_RX".into(),
        joy_code::RY => "DIJOFS_RY".into(),
        joy_code::RZ => "DIJOFS_RZ".into(),
        joy_code::STICK0 => "DIJOFS_STICK0".into(),
        joy_code::STICK1 => "DIJOFS_STICK1".into(),
        0x18 => "DIJOFS_SLIDER(0)".into(),
        0x1C => "DIJOFS_SLIDER(1)".into(),
        0x20 => "DIJOFS_POV0".into(),
        0x24 => "DIJOFS_POV(1)".into(),
        0x28 => "DIJOFS_POV2".into(),
        0x2C => "DIJOFS_POV3".into(),
        c if (joy_code::BUTTON0..joy_code::BUTTON0 + 32).contains(&c) => {
            let n = c - joy_code::BUTTON0;
            if [0, 3, 4, 6, 7, 8, 10, 12, 15, 17, 18, 19, 22, 28, 29, 31].contains(&n) {
                format!("DIJOFS_BUTTON({n})")
            } else {
                format!("DIJOFS_BUTTON{n}")
            }
        }
        _ => return None,
    })
}

/// The Tessera key of a keyboard code (§12.6.1), if §4.1 has one.
#[must_use]
pub fn keyboard_key(code: u16) -> Option<KeyCode> {
    KEYBOARD
        .iter()
        .find(|&&(_, c, _)| c == code)
        .and_then(|(_, _, t)| *t)
        .and_then(KeyCode::from_name)
}

/// The Tessera mouse button of a mouse code (§12.6.2).
#[must_use]
pub fn mouse_button(code: u16) -> Option<MouseButton> {
    Some(match code.checked_sub(mouse_code::BUTTON0)? {
        0 => MouseButton::Left,
        1 => MouseButton::Right,
        2 => MouseButton::Middle,
        3 => MouseButton::Back,
        4 => MouseButton::Forward,
        n @ 5..=7 => MouseButton::Other(n + 1),
        _ => return None,
    })
}

/// The Tessera pad button of an XInput slot's joystick button code
/// (§12.6.3); button 10 is `PadMode` (a Decision there), button 11 has no
/// counterpart.
#[must_use]
pub fn pad_button(code: u16) -> Option<PadButton> {
    Some(match code.checked_sub(joy_code::BUTTON0)? {
        0 => PadButton::South,
        1 => PadButton::East,
        2 => PadButton::West,
        3 => PadButton::North,
        4 => PadButton::LeftBumper,
        5 => PadButton::RightBumper,
        6 => PadButton::Select,
        7 => PadButton::Start,
        8 => PadButton::LeftStick,
        9 => PadButton::RightStick,
        10 => PadButton::Mode,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vocabulary_counts_and_spellings() {
        assert_eq!(KEYBOARD.len(), 137);
        assert_eq!(KEYBOARD_ALIASES.len(), 15);
        for (n, c, t) in KEYBOARD {
            assert_eq!(control_code(n.as_bytes()), Some(*c), "{n}");
            if let Some(t) = t {
                assert!(KeyCode::from_name(t).is_some(), "{t}");
            }
        }
        assert_eq!(control_code(b"dik_a"), None, "case-sensitive");
        assert_eq!(control_code(b"DIK_POWER"), None);
        assert_eq!(control_code(b"DIMOFS_WHEEL"), Some(8));
        assert_eq!(
            control_name(device_type::MOUSE, 8).as_deref(),
            Some("DIMOFS_Z")
        );
        assert_eq!(control_code(b"DIJOFS_BUTTON(5)"), Some(0x35));
        assert_eq!(control_code(b"DIJOFS_BUTTON31"), Some(0x4F));
        assert_eq!(control_code(b"DIJOFS_BUTTON32"), None);
        assert_eq!(control_code(b"DIJOFS_BUTTON05"), None);
        assert_eq!(control_code(b"DIJOFS_SLIDER(1)"), Some(0x1C));
        assert_eq!(control_code(b"DIJOFS_POV3"), Some(0x2C));
        assert_eq!(control_code(b"DIV_MOUSELOOK"), Some(1));
        assert_eq!(
            control_name(device_type::JOYSTICK, 0x30).as_deref(),
            Some("DIJOFS_BUTTON(0)")
        );
        assert_eq!(
            control_name(device_type::JOYSTICK, 0x31).as_deref(),
            Some("DIJOFS_BUTTON1")
        );
        assert_eq!(control_name(device_type::VIRTUAL, 1), None);
        assert_eq!(control_name(device_type::KEYBOARD, 0x400), None);
        // Every written joystick spelling reads back.
        for code in (0..0x50).chain([0x400, 0x404]) {
            if let Some(n) = control_name(device_type::JOYSTICK, code) {
                assert_eq!(control_code(n.as_bytes()), Some(code), "{n}");
            }
        }
        assert_eq!(keyboard_key(0x39), Some(KeyCode::Space));
        assert_eq!(keyboard_key(0x73), None);
        assert_eq!(mouse_button(0x13), Some(MouseButton::Other(8)));
        assert_eq!(pad_button(0x3A), Some(PadButton::Mode));
        assert_eq!(pad_button(0x3B), None);
        assert_eq!(sub_control_from_word(b"axispositive"), Some(1));
        assert_eq!(device_type_from_word(b"JOYSTICK"), Some(3));
    }
}
