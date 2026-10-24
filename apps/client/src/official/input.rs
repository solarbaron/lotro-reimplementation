// SPDX-License-Identifier: GPL-3.0-or-later
//! Device input (`docs/specs/official-entry.md` §2, §5.4 rule 1).
//!
//! A [`DeviceInput`] is a key, pointer or text event that the windowing system delivered to
//! the client's window. Its only constructor outside test builds,
//! `DeviceInput::from_window_event`, is called from one place: the window-event handler
//! of a start-screen run ([`super::window`], `EntryApp::window_event`, the windowing
//! library's per-window event callback); a source-scan test (OE-T7 (c)) fails if it is called
//! anywhere else outside `#[cfg(test)]` code. Synthetic sources (the windowed input script,
//! `--ui-script`, `--command` lines, the command dispatcher, the plugin host, scenario
//! drivers) feed the UI through other entry points and cannot make one; tests build values
//! with `DeviceInput::test`, which exists only in test builds of the client.

use std::time::Instant;

use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

/// Pixels of trackpad scrolling that count as one wheel line.
const PIXELS_PER_LINE: f64 = 40.0;

/// A key the official screens act on; every other key is [`Key::Other`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    /// Tab (with Shift: backwards).
    Tab,
    /// Enter or keypad Enter.
    Enter,
    /// Space.
    Space,
    /// Escape.
    Escape,
    /// Backspace.
    Backspace,
    /// Arrow up.
    Up,
    /// Arrow down.
    Down,
    /// Page up.
    PageUp,
    /// Page down.
    PageDown,
    /// Any other physical key.
    Other(PhysicalKey),
}

impl Key {
    fn from_physical(key: PhysicalKey) -> Self {
        match key {
            PhysicalKey::Code(KeyCode::Tab) => Self::Tab,
            PhysicalKey::Code(KeyCode::Enter | KeyCode::NumpadEnter) => Self::Enter,
            PhysicalKey::Code(KeyCode::Space) => Self::Space,
            PhysicalKey::Code(KeyCode::Escape) => Self::Escape,
            PhysicalKey::Code(KeyCode::Backspace) => Self::Backspace,
            PhysicalKey::Code(KeyCode::ArrowUp) => Self::Up,
            PhysicalKey::Code(KeyCode::ArrowDown) => Self::Down,
            PhysicalKey::Code(KeyCode::PageUp) => Self::PageUp,
            PhysicalKey::Code(KeyCode::PageDown) => Self::PageDown,
            other => Self::Other(other),
        }
    }
}

/// The modifier keys at the time of an event (kept by the window-event handler from
/// `ModifiersChanged`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers {
    /// Shift is down.
    pub shift: bool,
    /// Control is down.
    pub ctrl: bool,
    /// Alt (Option) is down. Control and Alt together are AltGr on Windows.
    pub alt: bool,
    /// The logo key (Command, Windows key) is down.
    pub logo: bool,
}

/// Most bytes of text one key press carries; longer text is cut at a character boundary.
pub const MAX_KEY_TEXT: usize = 32;

/// The text of one key press, held inline (never on the heap) and overwritten with zeros
/// when dropped: it may be part of a password (`official-entry.md` §7.3). No `Clone`; its
/// `Debug` form prints nothing of it.
pub struct KeyText {
    bytes: [u8; MAX_KEY_TEXT],
    len: u8,
}

impl KeyText {
    /// The first [`MAX_KEY_TEXT`] bytes of `s`, whole characters only.
    #[must_use]
    pub fn new(s: &str) -> Self {
        let mut bytes = [0u8; MAX_KEY_TEXT];
        let mut len = 0usize;
        for c in s.chars() {
            let n = c.len_utf8();
            let Some(dst) = bytes.get_mut(len..len + n) else {
                break;
            };
            c.encode_utf8(dst);
            len += n;
        }
        Self {
            bytes,
            len: u8::try_from(len).unwrap_or(0),
        }
    }

    /// The text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.bytes
            .get(..usize::from(self.len))
            .and_then(|b| std::str::from_utf8(b).ok())
            .unwrap_or("")
    }
}

impl KeyText {
    /// Overwrites the text with zeros (also done when it is dropped).
    pub fn clear(&mut self) {
        self.bytes.fill(0);
        std::hint::black_box(&self.bytes);
        self.len = 0;
    }

    /// Every byte (tests: the text reads back as zeros).
    #[cfg(test)]
    pub(crate) fn raw(&self) -> &[u8; MAX_KEY_TEXT] {
        &self.bytes
    }
}

impl Drop for KeyText {
    fn drop(&mut self) {
        self.clear();
    }
}

impl std::fmt::Debug for KeyText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<text>")
    }
}

/// What a device-input event is. No `Clone`: key text is never copied.
pub enum DeviceInputKind {
    /// A key went down or up.
    Key {
        /// The key.
        key: Key,
        /// Down (`true`) or up.
        pressed: bool,
        /// An automatic repeat of a held key.
        repeat: bool,
        /// The windowing system reported the key as already held when the window gained
        /// focus (not a press by the user now, §5.4 rule 2).
        held_before: bool,
        /// The modifiers.
        modifiers: Modifiers,
        /// The text the press produces, if any.
        text: Option<KeyText>,
    },
    /// The pointer moved to (`x`, `y`) in window pixels.
    PointerMoved {
        /// Horizontal position.
        x: f64,
        /// Vertical position.
        y: f64,
    },
    /// The primary pointer button went down or up.
    PointerButton {
        /// Down (`true`) or up.
        pressed: bool,
    },
    /// The wheel turned, in lines (positive: away from the user, scroll up).
    Wheel {
        /// Lines.
        lines: f32,
    },
    /// The user asked to close the window.
    CloseRequested,
    /// The window lost the keyboard focus (keys held now are released without an event).
    FocusLost,
}

impl std::fmt::Debug for DeviceInputKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // The text of a key press may be part of a password: never printed.
            Self::Key {
                key,
                pressed,
                repeat,
                held_before,
                modifiers,
                text,
            } => f
                .debug_struct("Key")
                .field("key", key)
                .field("pressed", pressed)
                .field("repeat", repeat)
                .field("held_before", held_before)
                .field("modifiers", modifiers)
                .field("text", text)
                .finish(),
            Self::PointerMoved { x, y } => f
                .debug_struct("PointerMoved")
                .field("x", x)
                .field("y", y)
                .finish(),
            Self::PointerButton { pressed } => f
                .debug_struct("PointerButton")
                .field("pressed", pressed)
                .finish(),
            Self::Wheel { lines } => f.debug_struct("Wheel").field("lines", lines).finish(),
            Self::CloseRequested => f.write_str("CloseRequested"),
            Self::FocusLost => f.write_str("FocusLost"),
        }
    }
}

/// One device-input event (module docs). No `Clone`.
#[derive(Debug)]
pub struct DeviceInput {
    kind: DeviceInputKind,
    at: Instant,
}

impl DeviceInput {
    /// What happened.
    #[must_use]
    pub fn kind(&self) -> &DeviceInputKind {
        &self.kind
    }

    /// When the window-event handler received it.
    #[must_use]
    pub fn at(&self) -> Instant {
        self.at
    }

    /// The device-input value of a window event, or `None` for events that are not key,
    /// pointer, wheel, focus-loss or close events. Called only by the window-event handler
    /// (module docs).
    pub(crate) fn from_window_event(
        event: &WindowEvent,
        modifiers: Modifiers,
        at: Instant,
    ) -> Option<Self> {
        let kind = match event {
            WindowEvent::KeyboardInput {
                event,
                is_synthetic,
                ..
            } => DeviceInputKind::Key {
                key: Key::from_physical(event.physical_key),
                pressed: event.state == ElementState::Pressed,
                repeat: event.repeat,
                held_before: *is_synthetic,
                modifiers,
                text: (event.state == ElementState::Pressed && !*is_synthetic)
                    .then(|| event.text.as_ref().map(|t| KeyText::new(t.as_str())))
                    .flatten(),
            },
            WindowEvent::CursorMoved { position, .. } => DeviceInputKind::PointerMoved {
                x: position.x,
                y: position.y,
            },
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => DeviceInputKind::PointerButton {
                pressed: *state == ElementState::Pressed,
            },
            WindowEvent::MouseWheel { delta, .. } => DeviceInputKind::Wheel {
                lines: match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y,
                    MouseScrollDelta::PixelDelta(p) => (p.y / PIXELS_PER_LINE) as f32,
                },
            },
            WindowEvent::CloseRequested => DeviceInputKind::CloseRequested,
            WindowEvent::Focused(false) => DeviceInputKind::FocusLost,
            _ => return None,
        };
        Some(Self { kind, at })
    }

    /// A device-input value for tests (exists only in test builds of the client).
    #[cfg(test)]
    pub(crate) fn test(kind: DeviceInputKind, at: Instant) -> Self {
        Self { kind, at }
    }
}
