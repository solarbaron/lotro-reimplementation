// SPDX-License-Identifier: GPL-3.0-or-later
//! Adapter from winit 0.30 events to [`RawEvent`]s (feature `winit`).
//!
//! ```ignore
//! fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
//!     let now = self.start.elapsed();
//!     tessera_input::winit_adapter::window_event(&event, |e| self.input.handle(&e, now));
//! }
//! fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
//!     let now = self.start.elapsed();
//!     tessera_input::winit_adapter::device_event(&event, |e| self.input.handle(&e, now));
//! }
//! ```

use winit::event::{DeviceEvent, ElementState, MouseScrollDelta, WindowEvent};
use winit::keyboard::{KeyCode as W, ModifiersState, PhysicalKey};

use crate::device::{KeyCode, Modifiers, MouseButton};
use crate::mapper::RawEvent;

/// Pixels of a touchpad/high-resolution scroll that count as one wheel
/// notch (Tessera's choice).
pub const PIXELS_PER_NOTCH: f64 = 40.0;
/// Most characters forwarded from one key event's text.
pub const MAX_TEXT_PER_KEY: usize = 32;

/// Translate a window event; calls `push` zero or more times.
pub fn window_event(event: &WindowEvent, mut push: impl FnMut(RawEvent)) {
    match event {
        WindowEvent::KeyboardInput { event, .. } => key_input(
            event.physical_key,
            event.state,
            event.repeat,
            event.text.as_deref(),
            push,
        ),
        WindowEvent::ModifiersChanged(m) => push(RawEvent::Modifiers(modifiers(m.state()))),
        WindowEvent::MouseInput { state, button, .. } => push(RawEvent::MouseButton {
            button: mouse_button(*button),
            pressed: *state == ElementState::Pressed,
        }),
        WindowEvent::MouseWheel { delta, .. } => push(wheel(*delta)),
        WindowEvent::CursorMoved { position, .. } => push(RawEvent::CursorMoved {
            x: position.x,
            y: position.y,
        }),
        WindowEvent::CursorLeft { .. } => push(RawEvent::CursorLeft),
        WindowEvent::Focused(false) => push(RawEvent::FocusLost),
        _ => {}
    }
}

/// Translate a device event (raw mouse motion for mouse look).
pub fn device_event(event: &DeviceEvent, mut push: impl FnMut(RawEvent)) {
    if let DeviceEvent::MouseMotion { delta } = event {
        push(RawEvent::MouseMotion {
            dx: delta.0,
            dy: delta.1,
        });
    }
}

/// Translate the parts of a keyboard event: the physical key (unmapped
/// keys are dropped), its state and auto-repeat flag, and on a press up to
/// [`MAX_TEXT_PER_KEY`] typed characters.
pub fn key_input(
    physical: PhysicalKey,
    state: ElementState,
    repeat: bool,
    text: Option<&str>,
    mut push: impl FnMut(RawEvent),
) {
    let pressed = state == ElementState::Pressed;
    if let PhysicalKey::Code(code) = physical {
        match key_code(code) {
            Some(key) => push(RawEvent::Key {
                key,
                pressed,
                repeat,
            }),
            None => tracing::trace!(?code, "unmapped key"),
        }
    }
    if pressed && let Some(text) = text {
        for c in text.chars().take(MAX_TEXT_PER_KEY) {
            push(RawEvent::Text(c));
        }
    }
}

/// Map winit's modifier state.
#[must_use]
pub fn modifiers(s: ModifiersState) -> Modifiers {
    let mut out = Modifiers::empty();
    out.set(Modifiers::SHIFT, s.shift_key());
    out.set(Modifiers::CTRL, s.control_key());
    out.set(Modifiers::ALT, s.alt_key());
    out.set(Modifiers::SUPER, s.super_key());
    out
}

/// Map a wheel delta to notches. winit's positive `x` scrolls content
/// right, which is a tilt to the **left**; Tessera's `WheelX` is `+` right
/// (§4.3), so `x` is negated. Positive `y` (content down) is the wheel
/// turned away from the user, which is Tessera's `+Y` as is.
#[must_use]
pub fn wheel(delta: MouseScrollDelta) -> RawEvent {
    match delta {
        MouseScrollDelta::LineDelta(x, y) => RawEvent::Wheel { dx: -x, dy: y },
        MouseScrollDelta::PixelDelta(p) => RawEvent::Wheel {
            dx: (-p.x / PIXELS_PER_NOTCH) as f32,
            dy: (p.y / PIXELS_PER_NOTCH) as f32,
        },
    }
}

/// Map a winit mouse button.
#[must_use]
pub fn mouse_button(b: winit::event::MouseButton) -> MouseButton {
    use winit::event::MouseButton as M;
    match b {
        M::Left => MouseButton::Left,
        M::Right => MouseButton::Right,
        M::Middle => MouseButton::Middle,
        M::Back => MouseButton::Back,
        M::Forward => MouseButton::Forward,
        M::Other(n) => MouseButton::Other(n),
    }
}

/// Map a winit physical key code; `None` for keys Tessera does not name.
#[must_use]
pub fn key_code(code: W) -> Option<KeyCode> {
    use KeyCode as K;
    Some(match code {
        W::KeyA => K::A,
        W::KeyB => K::B,
        W::KeyC => K::C,
        W::KeyD => K::D,
        W::KeyE => K::E,
        W::KeyF => K::F,
        W::KeyG => K::G,
        W::KeyH => K::H,
        W::KeyI => K::I,
        W::KeyJ => K::J,
        W::KeyK => K::K,
        W::KeyL => K::L,
        W::KeyM => K::M,
        W::KeyN => K::N,
        W::KeyO => K::O,
        W::KeyP => K::P,
        W::KeyQ => K::Q,
        W::KeyR => K::R,
        W::KeyS => K::S,
        W::KeyT => K::T,
        W::KeyU => K::U,
        W::KeyV => K::V,
        W::KeyW => K::W,
        W::KeyX => K::X,
        W::KeyY => K::Y,
        W::KeyZ => K::Z,
        W::Digit0 => K::Digit0,
        W::Digit1 => K::Digit1,
        W::Digit2 => K::Digit2,
        W::Digit3 => K::Digit3,
        W::Digit4 => K::Digit4,
        W::Digit5 => K::Digit5,
        W::Digit6 => K::Digit6,
        W::Digit7 => K::Digit7,
        W::Digit8 => K::Digit8,
        W::Digit9 => K::Digit9,
        W::F1 => K::F1,
        W::F2 => K::F2,
        W::F3 => K::F3,
        W::F4 => K::F4,
        W::F5 => K::F5,
        W::F6 => K::F6,
        W::F7 => K::F7,
        W::F8 => K::F8,
        W::F9 => K::F9,
        W::F10 => K::F10,
        W::F11 => K::F11,
        W::F12 => K::F12,
        W::F13 => K::F13,
        W::F14 => K::F14,
        W::F15 => K::F15,
        W::F16 => K::F16,
        W::F17 => K::F17,
        W::F18 => K::F18,
        W::F19 => K::F19,
        W::F20 => K::F20,
        W::F21 => K::F21,
        W::F22 => K::F22,
        W::F23 => K::F23,
        W::F24 => K::F24,
        W::Escape => K::Escape,
        W::Tab => K::Tab,
        W::CapsLock => K::CapsLock,
        W::Space => K::Space,
        W::Enter => K::Enter,
        W::Backspace => K::Backspace,
        W::Insert => K::Insert,
        W::Delete => K::Delete,
        W::Home => K::Home,
        W::End => K::End,
        W::PageUp => K::PageUp,
        W::PageDown => K::PageDown,
        W::ArrowUp => K::Up,
        W::ArrowDown => K::Down,
        W::ArrowLeft => K::Left,
        W::ArrowRight => K::Right,
        W::ShiftLeft => K::ShiftLeft,
        W::ShiftRight => K::ShiftRight,
        W::ControlLeft => K::ControlLeft,
        W::ControlRight => K::ControlRight,
        W::AltLeft => K::AltLeft,
        W::AltRight => K::AltRight,
        W::SuperLeft => K::SuperLeft,
        W::SuperRight => K::SuperRight,
        W::Backquote => K::Backquote,
        W::Minus => K::Minus,
        W::Equal => K::Equal,
        W::BracketLeft => K::BracketLeft,
        W::BracketRight => K::BracketRight,
        W::Backslash => K::Backslash,
        W::Semicolon => K::Semicolon,
        W::Quote => K::Quote,
        W::Comma => K::Comma,
        W::Period => K::Period,
        W::Slash => K::Slash,
        W::IntlBackslash => K::IntlBackslash,
        W::Numpad0 => K::Numpad0,
        W::Numpad1 => K::Numpad1,
        W::Numpad2 => K::Numpad2,
        W::Numpad3 => K::Numpad3,
        W::Numpad4 => K::Numpad4,
        W::Numpad5 => K::Numpad5,
        W::Numpad6 => K::Numpad6,
        W::Numpad7 => K::Numpad7,
        W::Numpad8 => K::Numpad8,
        W::Numpad9 => K::Numpad9,
        W::NumpadAdd => K::NumpadAdd,
        W::NumpadSubtract => K::NumpadSubtract,
        W::NumpadMultiply => K::NumpadMultiply,
        W::NumpadDivide => K::NumpadDivide,
        W::NumpadDecimal => K::NumpadDecimal,
        W::NumpadEnter => K::NumpadEnter,
        W::NumLock => K::NumLock,
        W::ScrollLock => K::ScrollLock,
        W::Pause => K::Pause,
        W::PrintScreen => K::PrintScreen,
        W::ContextMenu => K::ContextMenu,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_map_one_to_one() {
        let codes = [
            W::KeyA,
            W::KeyZ,
            W::Digit0,
            W::F24,
            W::ArrowUp,
            W::ShiftRight,
            W::NumpadEnter,
            W::ContextMenu,
            W::Backquote,
        ];
        let mut seen = std::collections::HashSet::new();
        for c in codes {
            let k = key_code(c).unwrap();
            assert!(seen.insert(k));
        }
        assert_eq!(key_code(W::F35), None);
    }

    #[test]
    fn wheel_and_buttons() {
        assert_eq!(
            wheel(MouseScrollDelta::PixelDelta((80.0, -40.0).into())),
            RawEvent::Wheel { dx: -2.0, dy: -1.0 }
        );
        // winit: a tilt to the left is LineDelta(+1, 0); away is (0, +1).
        assert_eq!(
            wheel(MouseScrollDelta::LineDelta(1.0, 0.0)),
            RawEvent::Wheel { dx: -1.0, dy: 0.0 }
        );
        assert_eq!(
            wheel(MouseScrollDelta::LineDelta(0.0, 1.0)),
            RawEvent::Wheel { dx: 0.0, dy: 1.0 }
        );
        assert_eq!(
            mouse_button(winit::event::MouseButton::Other(9)),
            MouseButton::Other(9)
        );
        let mut got = Vec::new();
        window_event(&WindowEvent::Focused(false), |e| got.push(e));
        device_event(&DeviceEvent::MouseMotion { delta: (1.0, 2.0) }, |e| {
            got.push(e)
        });
        assert_eq!(
            got,
            [
                RawEvent::FocusLost,
                RawEvent::MouseMotion { dx: 1.0, dy: 2.0 }
            ]
        );
    }

    #[test]
    fn tilt_left_clicks_wheel_left() {
        use crate::{ActionDef, ContextDef, InputMapper, Keymap, Registry};
        let mut r = Registry::new();
        let left = r.add_action(ActionDef::button("left")).unwrap();
        let right = r.add_action(ActionDef::button("right")).unwrap();
        let c = r.add_context(ContextDef::new("c", 0)).unwrap();
        let km =
            Keymap::parse("[bindings.c]\nleft = \"WheelLeft\"\nright = \"WheelRight\"\n").unwrap();
        let (mut m, _) = InputMapper::new(r, &km);
        m.activate(c);
        let t = std::time::Duration::ZERO;
        m.handle(&wheel(MouseScrollDelta::LineDelta(1.0, 0.0)), t);
        m.update(t);
        assert!(m.just_pressed(left) && !m.just_pressed(right));
    }

    #[test]
    fn keyboard_and_modifiers() {
        let mut got = Vec::new();
        let long = "x".repeat(100);
        key_input(
            PhysicalKey::Code(W::KeyW),
            ElementState::Pressed,
            true,
            Some(&long),
            |e| got.push(e),
        );
        assert_eq!(
            got[0],
            RawEvent::Key {
                key: KeyCode::W,
                pressed: true,
                repeat: true
            }
        );
        assert_eq!(got.len(), 1 + MAX_TEXT_PER_KEY, "text is capped");
        assert!(got[1..].iter().all(|e| *e == RawEvent::Text('x')));
        got.clear();
        // Releases carry no text; unmapped keys still forward their text.
        key_input(
            PhysicalKey::Code(W::KeyW),
            ElementState::Released,
            false,
            Some("w"),
            |e| got.push(e),
        );
        key_input(
            PhysicalKey::Code(W::F35),
            ElementState::Pressed,
            false,
            Some("é"),
            |e| got.push(e),
        );
        key_input(
            PhysicalKey::Unidentified(winit::keyboard::NativeKeyCode::Unidentified),
            ElementState::Pressed,
            false,
            None,
            |e| got.push(e),
        );
        assert_eq!(
            got,
            [
                RawEvent::Key {
                    key: KeyCode::W,
                    pressed: false,
                    repeat: false
                },
                RawEvent::Text('é')
            ]
        );
        let m = modifiers(ModifiersState::SHIFT | ModifiersState::SUPER);
        assert_eq!(m, Modifiers::SHIFT | Modifiers::SUPER);
        let mut got = Vec::new();
        window_event(
            &WindowEvent::ModifiersChanged(ModifiersState::CONTROL.into()),
            |e| got.push(e),
        );
        assert_eq!(got, [RawEvent::Modifiers(Modifiers::CTRL)]);
    }
}
