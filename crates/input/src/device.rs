// SPDX-License-Identifier: GPL-3.0-or-later
//! The device vocabulary: physical keys, mouse buttons, gamepad buttons,
//! analog axes and modifiers, with the names the keymap file uses for them
//! (`docs/specs/keymap.md` §4).
//!
//! Keys are **physical** positions named after a US layout (the key that
//! types `W` on a US keyboard is `W` everywhere), so movement bindings keep
//! their shape on other layouts.

use std::fmt;

/// Defines a fieldless enum with a canonical name and aliases per variant.
macro_rules! named_enum {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $( $(#[$vmeta:meta])* $var:ident => $canon:literal $(| $alias:literal)* , )*
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[non_exhaustive]
        $vis enum $name {
            $( $(#[$vmeta])* $var, )*
        }

        impl $name {
            /// Every variant, in declaration order.
            pub const ALL: &'static [Self] = &[ $( Self::$var, )* ];

            /// The canonical keymap name.
            #[must_use]
            pub const fn name(self) -> &'static str {
                match self { $( Self::$var => $canon, )* }
            }

            /// Look up a keymap name (canonical or alias), ASCII
            /// case-insensitively.
            #[must_use]
            pub fn from_name(text: &str) -> Option<Self> {
                $(
                    if text.eq_ignore_ascii_case($canon) $(|| text.eq_ignore_ascii_case($alias))* {
                        return Some(Self::$var);
                    }
                )*
                None
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.name())
            }
        }
    };
}

named_enum! {
    /// A physical keyboard key, named by its position on a US layout.
    pub enum KeyCode {
        A => "A" | "KeyA", B => "B" | "KeyB", C => "C" | "KeyC", D => "D" | "KeyD",
        E => "E" | "KeyE", F => "F" | "KeyF", G => "G" | "KeyG", H => "H" | "KeyH",
        I => "I" | "KeyI", J => "J" | "KeyJ", K => "K" | "KeyK", L => "L" | "KeyL",
        M => "M" | "KeyM", N => "N" | "KeyN", O => "O" | "KeyO", P => "P" | "KeyP",
        Q => "Q" | "KeyQ", R => "R" | "KeyR", S => "S" | "KeyS", T => "T" | "KeyT",
        U => "U" | "KeyU", V => "V" | "KeyV", W => "W" | "KeyW", X => "X" | "KeyX",
        Y => "Y" | "KeyY", Z => "Z" | "KeyZ",
        Digit0 => "0" | "Digit0", Digit1 => "1" | "Digit1", Digit2 => "2" | "Digit2",
        Digit3 => "3" | "Digit3", Digit4 => "4" | "Digit4", Digit5 => "5" | "Digit5",
        Digit6 => "6" | "Digit6", Digit7 => "7" | "Digit7", Digit8 => "8" | "Digit8",
        Digit9 => "9" | "Digit9",
        F1 => "F1", F2 => "F2", F3 => "F3", F4 => "F4", F5 => "F5", F6 => "F6",
        F7 => "F7", F8 => "F8", F9 => "F9", F10 => "F10", F11 => "F11", F12 => "F12",
        F13 => "F13", F14 => "F14", F15 => "F15", F16 => "F16", F17 => "F17", F18 => "F18",
        F19 => "F19", F20 => "F20", F21 => "F21", F22 => "F22", F23 => "F23", F24 => "F24",
        Escape => "Escape" | "Esc",
        Tab => "Tab",
        CapsLock => "CapsLock",
        Space => "Space",
        Enter => "Enter" | "Return",
        Backspace => "Backspace",
        Insert => "Insert" | "Ins",
        Delete => "Delete" | "Del",
        Home => "Home",
        End => "End",
        PageUp => "PageUp" | "PgUp",
        PageDown => "PageDown" | "PgDn",
        Up => "Up" | "ArrowUp",
        Down => "Down" | "ArrowDown",
        Left => "Left" | "ArrowLeft",
        Right => "Right" | "ArrowRight",
        ShiftLeft => "LShift" | "ShiftLeft",
        ShiftRight => "RShift" | "ShiftRight",
        ControlLeft => "LCtrl" | "ControlLeft" | "CtrlLeft",
        ControlRight => "RCtrl" | "ControlRight" | "CtrlRight",
        AltLeft => "LAlt" | "AltLeft",
        AltRight => "RAlt" | "AltRight" | "AltGr",
        SuperLeft => "LSuper" | "SuperLeft" | "MetaLeft",
        SuperRight => "RSuper" | "SuperRight" | "MetaRight",
        Backquote => "Backquote" | "Grave" | "Tilde",
        Minus => "Minus",
        Equal => "Equal" | "Equals",
        BracketLeft => "BracketLeft" | "LBracket",
        BracketRight => "BracketRight" | "RBracket",
        Backslash => "Backslash",
        Semicolon => "Semicolon",
        Quote => "Quote" | "Apostrophe",
        Comma => "Comma",
        Period => "Period" | "Dot",
        Slash => "Slash",
        IntlBackslash => "IntlBackslash",
        Numpad0 => "Numpad0", Numpad1 => "Numpad1", Numpad2 => "Numpad2",
        Numpad3 => "Numpad3", Numpad4 => "Numpad4", Numpad5 => "Numpad5",
        Numpad6 => "Numpad6", Numpad7 => "Numpad7", Numpad8 => "Numpad8",
        Numpad9 => "Numpad9",
        NumpadAdd => "NumpadAdd" | "NumpadPlus",
        NumpadSubtract => "NumpadSubtract" | "NumpadMinus",
        NumpadMultiply => "NumpadMultiply" | "NumpadStar",
        NumpadDivide => "NumpadDivide" | "NumpadSlash",
        NumpadDecimal => "NumpadDecimal" | "NumpadPeriod",
        NumpadEnter => "NumpadEnter",
        NumLock => "NumLock",
        ScrollLock => "ScrollLock",
        Pause => "Pause",
        PrintScreen => "PrintScreen" | "PrtSc",
        ContextMenu => "ContextMenu" | "Menu",
    }
}

impl KeyCode {
    /// The modifier this key contributes while held, if it is a modifier key.
    #[must_use]
    pub const fn modifier(self) -> Modifiers {
        match self {
            Self::ShiftLeft | Self::ShiftRight => Modifiers::SHIFT,
            Self::ControlLeft | Self::ControlRight => Modifiers::CTRL,
            Self::AltLeft | Self::AltRight => Modifiers::ALT,
            Self::SuperLeft | Self::SuperRight => Modifiers::SUPER,
            _ => Modifiers::empty(),
        }
    }
}

named_enum! {
    /// A digital gamepad button (positional names: `South` is the bottom face
    /// button whatever its label).
    pub enum PadButton {
        South => "PadSouth" | "PadA",
        East => "PadEast" | "PadB",
        West => "PadWest" | "PadX",
        North => "PadNorth" | "PadY",
        LeftBumper => "PadL1" | "PadLeftBumper",
        RightBumper => "PadR1" | "PadRightBumper",
        LeftTrigger => "PadL2" | "PadLeftTriggerButton",
        RightTrigger => "PadR2" | "PadRightTriggerButton",
        Select => "PadSelect" | "PadBack",
        Start => "PadStart",
        Mode => "PadMode" | "PadGuide",
        LeftStick => "PadL3" | "PadLeftStick",
        RightStick => "PadR3" | "PadRightStick",
        DPadUp => "PadUp",
        DPadDown => "PadDown",
        DPadLeft => "PadLeft",
        DPadRight => "PadRight",
    }
}

named_enum! {
    /// A continuous input source.
    ///
    /// *Relative* axes ([`AnalogAxis::is_relative`]) report movement since
    /// the last frame (mouse motion in device units, wheel in notches) and
    /// feed an action's `delta`. *Absolute* axes report a position in
    /// `-1..=1` (sticks) or `0..=1` (triggers) and feed an action's `value`.
    pub enum AnalogAxis {
        MouseX => "MouseX",
        MouseY => "MouseY",
        WheelX => "WheelX",
        WheelY => "WheelY",
        PadLeftX => "PadLeftX",
        PadLeftY => "PadLeftY",
        PadRightX => "PadRightX",
        PadRightY => "PadRightY",
        PadLeftTrigger => "PadLeftTrigger" | "PadLT",
        PadRightTrigger => "PadRightTrigger" | "PadRT",
    }
}

impl AnalogAxis {
    /// Whether the axis reports per-frame movement rather than a position.
    #[must_use]
    pub const fn is_relative(self) -> bool {
        matches!(
            self,
            Self::MouseX | Self::MouseY | Self::WheelX | Self::WheelY
        )
    }

    /// Dense index, `0..AnalogAxis::ALL.len()`.
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// A mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MouseButton {
    /// Primary button.
    Left,
    /// Secondary button.
    Right,
    /// Wheel button.
    Middle,
    /// "Back" side button.
    Back,
    /// "Forward" side button.
    Forward,
    /// Any other button, numbered as the platform reports it
    /// ([`MouseButton::MIN_OTHER`]..=[`MouseButton::MAX_OTHER`] are nameable
    /// in a keymap as `Mouse6`..`Mouse65535`; `Other(0..=5)` has no name
    /// because `Mouse1`..`Mouse5` name the five standard buttons).
    Other(u16),
}

impl MouseButton {
    /// Lowest platform number with a keymap name (`Mouse6`).
    pub const MIN_OTHER: u16 = 6;
    /// Highest platform number with a keymap name (`Mouse65535`): every
    /// number a platform can report from 6 up is nameable, so a captured
    /// button can always be saved.
    pub const MAX_OTHER: u16 = u16::MAX;

    /// Keymap name; `None` for an `Other` number outside the nameable range.
    #[must_use]
    pub fn name(self) -> Option<String> {
        Some(match self {
            Self::Left => "MouseLeft".into(),
            Self::Right => "MouseRight".into(),
            Self::Middle => "MouseMiddle".into(),
            Self::Back => "MouseBack".into(),
            Self::Forward => "MouseForward".into(),
            Self::Other(n) if (Self::MIN_OTHER..=Self::MAX_OTHER).contains(&n) => {
                format!("Mouse{n}")
            }
            Self::Other(_) => return None,
        })
    }

    /// Parse a keymap name (ASCII case-insensitive).
    #[must_use]
    pub fn from_name(text: &str) -> Option<Self> {
        let named = [
            ("MouseLeft", Self::Left),
            ("Mouse1", Self::Left),
            ("MouseRight", Self::Right),
            ("Mouse2", Self::Right),
            ("MouseMiddle", Self::Middle),
            ("Mouse3", Self::Middle),
            ("MouseBack", Self::Back),
            ("Mouse4", Self::Back),
            ("MouseForward", Self::Forward),
            ("Mouse5", Self::Forward),
        ];
        if let Some(&(_, b)) = named.iter().find(|(n, _)| text.eq_ignore_ascii_case(n)) {
            return Some(b);
        }
        let head = text.get(..5)?;
        if !head.eq_ignore_ascii_case("Mouse") {
            return None;
        }
        let digits = &text[5..];
        if digits.is_empty() || digits.len() > 5 || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let n: u16 = digits.parse().ok()?;
        (Self::MIN_OTHER..=Self::MAX_OTHER)
            .contains(&n)
            .then_some(Self::Other(n))
    }
}

/// Wheel "clicks" usable as momentary buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum WheelDirection {
    /// Away from the user.
    Up,
    /// Towards the user.
    Down,
    /// Tilt left.
    Left,
    /// Tilt right.
    Right,
}

impl WheelDirection {
    const NAMES: [(&'static str, Self); 4] = [
        ("WheelUp", Self::Up),
        ("WheelDown", Self::Down),
        ("WheelLeft", Self::Left),
        ("WheelRight", Self::Right),
    ];

    /// Keymap name.
    #[must_use]
    pub fn name(self) -> &'static str {
        Self::NAMES
            .iter()
            .find(|(_, d)| *d == self)
            .map_or("WheelUp", |(n, _)| n)
    }

    /// Parse a keymap name (ASCII case-insensitive).
    #[must_use]
    pub fn from_name(text: &str) -> Option<Self> {
        Self::NAMES
            .iter()
            .find(|(n, _)| text.eq_ignore_ascii_case(n))
            .map(|&(_, d)| d)
    }
}

/// Anything with a pressed/released state that can appear in a stroke.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Button {
    /// A keyboard key.
    Key(KeyCode),
    /// A mouse button.
    Mouse(MouseButton),
    /// A gamepad button.
    Pad(PadButton),
    /// A wheel notch, which presses and releases in the same instant.
    Wheel(WheelDirection),
}

impl Button {
    /// Keymap name, `None` for a mouse button with no name.
    #[must_use]
    pub fn name(self) -> Option<String> {
        match self {
            Self::Key(k) => Some(k.name().into()),
            Self::Mouse(m) => m.name(),
            Self::Pad(p) => Some(p.name().into()),
            Self::Wheel(w) => Some(w.name().into()),
        }
    }

    /// Parse a keymap name (keys, mouse buttons, pad buttons, wheel clicks).
    #[must_use]
    pub fn from_name(text: &str) -> Option<Self> {
        KeyCode::from_name(text)
            .map(Self::Key)
            .or_else(|| MouseButton::from_name(text).map(Self::Mouse))
            .or_else(|| PadButton::from_name(text).map(Self::Pad))
            .or_else(|| WheelDirection::from_name(text).map(Self::Wheel))
    }

    /// The modifier this button contributes while held (only modifier keys
    /// contribute one).
    #[must_use]
    pub const fn modifier(self) -> Modifiers {
        match self {
            Self::Key(k) => k.modifier(),
            _ => Modifiers::empty(),
        }
    }
}

impl fmt::Display for Button {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.name() {
            Some(n) => f.write_str(&n),
            None => write!(f, "{self:?}"),
        }
    }
}

bitflags::bitflags! {
    /// Logical modifier state (left and right keys are merged).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
    pub struct Modifiers: u8 {
        /// Either Control key.
        const CTRL = 1;
        /// Either Alt key.
        const ALT = 1 << 1;
        /// Either Shift key.
        const SHIFT = 1 << 2;
        /// Either Super (Windows/Command) key.
        const SUPER = 1 << 3;
    }
}

impl Modifiers {
    /// Keymap tokens in canonical order, with aliases.
    const TOKENS: [(&'static str, Self); 7] = [
        ("Ctrl", Self::CTRL),
        ("Alt", Self::ALT),
        ("Shift", Self::SHIFT),
        ("Super", Self::SUPER),
        ("Control", Self::CTRL),
        ("Meta", Self::SUPER),
        ("Cmd", Self::SUPER),
    ];

    /// Parse one modifier token (`Ctrl`, `Alt`, `Shift`, `Super` and aliases).
    #[must_use]
    pub fn from_token(text: &str) -> Option<Self> {
        Self::TOKENS
            .iter()
            .find(|(n, _)| text.eq_ignore_ascii_case(n))
            .map(|&(_, m)| m)
    }

    /// Canonical tokens of the set flags, in `Ctrl`, `Alt`, `Shift`, `Super`
    /// order.
    pub fn tokens(self) -> impl Iterator<Item = &'static str> {
        Self::TOKENS[..4]
            .iter()
            .filter(move |(_, m)| self.contains(*m))
            .map(|(n, _)| *n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_name_round_trips() {
        for &k in KeyCode::ALL {
            assert_eq!(KeyCode::from_name(k.name()), Some(k), "{k:?}");
            assert_eq!(Button::from_name(k.name()), Some(Button::Key(k)));
        }
        for &p in PadButton::ALL {
            assert_eq!(Button::from_name(p.name()), Some(Button::Pad(p)));
        }
        for &a in AnalogAxis::ALL {
            assert_eq!(AnalogAxis::from_name(a.name()), Some(a));
            assert_eq!(
                Button::from_name(a.name()),
                None,
                "{a:?} must not be a button"
            );
        }
        for n in (0..=40u16).chain([275, 999, 65535]) {
            let b = MouseButton::Other(n);
            match b.name() {
                Some(name) => assert_eq!(MouseButton::from_name(&name), Some(b)),
                None => assert!(n < MouseButton::MIN_OTHER, "{n} must be nameable"),
            }
        }
        for b in [
            MouseButton::Left,
            MouseButton::Right,
            MouseButton::Middle,
            MouseButton::Back,
            MouseButton::Forward,
        ] {
            assert_eq!(MouseButton::from_name(&b.name().unwrap()), Some(b));
        }
    }

    #[test]
    fn names_are_unambiguous() {
        let mut seen = std::collections::HashSet::new();
        let mut add = |n: &str| assert!(seen.insert(n.to_ascii_lowercase()), "dup {n}");
        for &k in KeyCode::ALL {
            add(k.name());
        }
        for &p in PadButton::ALL {
            add(p.name());
        }
        for &a in AnalogAxis::ALL {
            add(a.name());
        }
        for (n, _) in Modifiers::TOKENS {
            add(n);
        }
        for (n, _) in WheelDirection::NAMES {
            add(n);
        }
    }

    #[test]
    fn aliases_and_case() {
        assert_eq!(KeyCode::from_name("esc"), Some(KeyCode::Escape));
        assert_eq!(KeyCode::from_name("keyw"), Some(KeyCode::W));
        assert_eq!(
            MouseButton::from_name("mouse7"),
            Some(MouseButton::Other(7))
        );
        assert_eq!(
            MouseButton::from_name("Mouse33"),
            Some(MouseButton::Other(33))
        );
        assert_eq!(
            MouseButton::from_name("mouse00275"),
            Some(MouseButton::Other(275))
        );
        assert_eq!(MouseButton::from_name("Mouse65536"), None);
        assert_eq!(MouseButton::from_name("Mouse100000"), None);
        assert_eq!(MouseButton::from_name("Mouse0"), None);
        assert_eq!(MouseButton::from_name("Mouse"), None);
        assert_eq!(MouseButton::from_name("Mouse+1"), None);
        assert_eq!(MouseButton::from_name("Mouseé"), None);
        assert_eq!(Modifiers::from_token("control"), Some(Modifiers::CTRL));
        let all = Modifiers::all();
        assert_eq!(
            all.tokens().collect::<Vec<_>>(),
            ["Ctrl", "Alt", "Shift", "Super"]
        );
    }
}
