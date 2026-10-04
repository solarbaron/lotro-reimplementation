// SPDX-License-Identifier: GPL-3.0-or-later
//! Tessera's built-in actions, contexts and default bindings
//! (`docs/specs/keymap.md` §8), plus the glue from action states to
//! controller intents.
//!
//! The layout is Tessera's own choice for a third-person world walker
//! (WASD-style movement with A/D turning, Q/E strafing, both mouse buttons
//! to run forward); it is not copied from any other program's key map.

use crate::action::{ActionDef, ActionId, ContextDef, ContextId, ModifierPolicy, Registry};
use crate::controller::MoveIntent;
use crate::device::AnalogAxis;
use crate::keymap::{BindingSpec, Keymap};
use crate::mapper::InputMapper;

/// Context names and priorities.
pub mod contexts {
    /// Always-on commands (menu, screenshot, mode toggles). Priority 0.
    pub const GLOBAL: &str = "global";
    /// Camera look and zoom. Priority 10.
    pub const CAMERA: &str = "camera";
    /// Walking. Priority 20.
    pub const MOVEMENT: &str = "movement";
    /// Free flight; active instead of or above movement. Priority 30.
    pub const FLY: &str = "fly";
    /// Text entry (chat line); opaque, receives text. Priority 100.
    pub const TEXT_ENTRY: &str = "text_entry";
}

/// The built-in actions and their kinds, `(name, def)`.
fn action_defs() -> Vec<ActionDef> {
    let sub = ModifierPolicy::Subset;
    vec![
        // global
        ActionDef::button("menu"),
        ActionDef::button("screenshot"),
        ActionDef::button("toggle_fly"),
        ActionDef::button("toggle_debug"),
        ActionDef::button("open_chat"),
        // camera
        ActionDef::button("mouselook").with_modifiers(sub),
        ActionDef::axis("look_x"),
        ActionDef::axis("look_y"),
        ActionDef::axis("zoom"),
        ActionDef::button("reset_camera"),
        // movement
        ActionDef::axis("move_forward"),
        ActionDef::axis("strafe"),
        ActionDef::axis("turn"),
        ActionDef::button("jump").with_modifiers(sub),
        ActionDef::button("walk").with_modifiers(sub),
        ActionDef::button("autorun").with_modifiers(sub),
        // fly
        ActionDef::axis("fly_vertical"),
        ActionDef::button("fly_boost").with_modifiers(sub),
        ActionDef::button("fly_slow").with_modifiers(sub),
        ActionDef::axis("fly_speed"),
        // text entry
        ActionDef::button("text_submit"),
        ActionDef::button("text_cancel"),
        ActionDef::button("text_backspace"),
        ActionDef::button("text_history_prev"),
        ActionDef::button("text_history_next"),
    ]
}

/// A registry with every built-in context and action.
#[must_use]
pub fn default_registry() -> Registry {
    let mut r = Registry::new();
    let contexts = [
        ContextDef::new(contexts::GLOBAL, 0),
        ContextDef::new(contexts::CAMERA, 10),
        ContextDef::new(contexts::MOVEMENT, 20),
        ContextDef::new(contexts::FLY, 30),
        ContextDef::new(contexts::TEXT_ENTRY, 100)
            .opaque()
            .with_text(),
    ];
    for c in contexts {
        // Built-in names are valid and unique; a failure would be a bug
        // caught by the tests, never by users.
        if let Err(e) = r.add_context(c) {
            tracing::error!(error = %e, "built-in context rejected");
        }
    }
    for a in action_defs() {
        if let Err(e) = r.add_action(a) {
            tracing::error!(error = %e, "built-in action rejected");
        }
    }
    r
}

/// Built-in default bindings, as `(context, action, bindings)`.
#[allow(clippy::type_complexity)]
fn default_table() -> Vec<(&'static str, &'static str, Vec<Entry>)> {
    use Entry::{A, D, V};
    use contexts::{CAMERA, FLY, GLOBAL, MOVEMENT, TEXT_ENTRY};
    vec![
        (GLOBAL, "menu", vec![D("Escape")]),
        (GLOBAL, "screenshot", vec![D("PrintScreen"), D("F12")]),
        (GLOBAL, "toggle_fly", vec![D("Ctrl+F")]),
        (GLOBAL, "toggle_debug", vec![D("F3")]),
        (GLOBAL, "open_chat", vec![D("Enter"), D("NumpadEnter")]),
        (CAMERA, "mouselook", vec![D("MouseRight")]),
        (
            CAMERA,
            "look_x",
            vec![A("MouseX", 1.0, 0.0), A("PadRightX", 1.0, 0.15)],
        ),
        (
            CAMERA,
            "look_y",
            vec![A("MouseY", 1.0, 0.0), A("PadRightY", -1.0, 0.15)],
        ),
        (
            CAMERA,
            "zoom",
            vec![A("WheelY", 1.0, 0.0), V("PageUp", 1.0), V("PageDown", -1.0)],
        ),
        (CAMERA, "reset_camera", vec![D("Home")]),
        (
            MOVEMENT,
            "move_forward",
            vec![
                D("W"),
                D("Up"),
                D("MouseLeft+MouseRight"),
                V("S", -1.0),
                V("Down", -1.0),
                A("PadLeftY", 1.0, 0.15),
            ],
        ),
        (
            MOVEMENT,
            "strafe",
            vec![V("E", 1.0), V("Q", -1.0), A("PadLeftX", 1.0, 0.15)],
        ),
        (
            MOVEMENT,
            "turn",
            vec![V("A", 1.0), V("Left", 1.0), V("D", -1.0), V("Right", -1.0)],
        ),
        // Mouse look is bound here as well as in camera: a press of the
        // right button that completes the `MouseLeft+MouseRight` run chord
        // is consumed by this context, so camera's binding would never see
        // it. Bound here, the single-button binding fires together with the
        // chord (same context, §7.2), and its hold outlives the chord when
        // the left button is let go, whatever the press order.
        (MOVEMENT, "mouselook", vec![D("MouseRight")]),
        (MOVEMENT, "jump", vec![D("Space"), D("PadSouth")]),
        (MOVEMENT, "walk", vec![D("Shift")]),
        (MOVEMENT, "autorun", vec![D("NumLock"), D("R"), D("PadL3")]),
        (
            FLY,
            "fly_vertical",
            vec![
                V("Space", 1.0),
                V("C", -1.0),
                V("PadR1", 1.0),
                V("PadL1", -1.0),
            ],
        ),
        (FLY, "fly_boost", vec![D("Shift")]),
        (FLY, "fly_slow", vec![D("Alt")]),
        (FLY, "fly_speed", vec![A("WheelY", 1.0, 0.0)]),
        (
            TEXT_ENTRY,
            "text_submit",
            vec![D("Enter"), D("NumpadEnter")],
        ),
        (TEXT_ENTRY, "text_cancel", vec![D("Escape")]),
        (TEXT_ENTRY, "text_backspace", vec![D("Backspace")]),
        (TEXT_ENTRY, "text_history_prev", vec![D("Up")]),
        (TEXT_ENTRY, "text_history_next", vec![D("Down")]),
    ]
}

enum Entry {
    /// Digital, value 1.
    D(&'static str),
    /// Digital with a value.
    V(&'static str, f32),
    /// Analog `(axis, scale, deadzone)`.
    A(&'static str, f32, f32),
}

/// The built-in default keymap (every built-in action bound).
#[must_use]
pub fn default_keymap() -> Keymap {
    let mut k = Keymap::new();
    for (ctx, action, entries) in default_table() {
        let list = entries
            .into_iter()
            .filter_map(|e| match e {
                Entry::D(t) => BindingSpec::parse(t).ok(),
                Entry::V(t, v) => crate::Chord::parse(t)
                    .ok()
                    .map(|c| BindingSpec::digital(c, v)),
                Entry::A(a, s, d) => {
                    AnalogAxis::from_name(a).map(|axis| BindingSpec::analog(axis, s, d))
                }
            })
            .collect();
        if let Err(e) = k.set(ctx, action, list) {
            tracing::error!(error = %e, "built-in binding rejected");
        }
    }
    k
}

/// Ids of the built-in actions and contexts, looked up once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)]
pub struct StandardActions {
    pub global: ContextId,
    pub camera: ContextId,
    pub movement: ContextId,
    pub fly: ContextId,
    pub text_entry: ContextId,
    pub menu: ActionId,
    pub screenshot: ActionId,
    pub toggle_fly: ActionId,
    pub toggle_debug: ActionId,
    pub open_chat: ActionId,
    pub mouselook: ActionId,
    pub look_x: ActionId,
    pub look_y: ActionId,
    pub zoom: ActionId,
    pub reset_camera: ActionId,
    pub move_forward: ActionId,
    pub strafe: ActionId,
    pub turn: ActionId,
    pub jump: ActionId,
    pub walk: ActionId,
    pub autorun: ActionId,
    pub fly_vertical: ActionId,
    pub fly_boost: ActionId,
    pub fly_slow: ActionId,
    pub fly_speed: ActionId,
    pub text_submit: ActionId,
    pub text_cancel: ActionId,
    pub text_backspace: ActionId,
    pub text_history_prev: ActionId,
    pub text_history_next: ActionId,
}

/// Look-input tuning for [`StandardActions::look`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LookSettings {
    /// Radians per mouse device unit.
    pub mouse_sensitivity: f32,
    /// Radians per second at full stick deflection.
    pub stick_rate: f32,
    /// Invert vertical look.
    pub invert_y: bool,
}

impl Default for LookSettings {
    /// Tessera's own defaults: 0.0025 rad per device unit, 2.5 rad/s stick.
    fn default() -> Self {
        Self {
            mouse_sensitivity: 0.0025,
            stick_rate: 2.5,
            invert_y: false,
        }
    }
}

impl StandardActions {
    /// Look up every built-in id in `registry`; `None` if one is missing
    /// (the registry was not built from [`default_registry`]).
    #[must_use]
    pub fn new(registry: &Registry) -> Option<Self> {
        let c = |n| registry.context(n);
        let a = |n| registry.action(n);
        Some(Self {
            global: c(contexts::GLOBAL)?,
            camera: c(contexts::CAMERA)?,
            movement: c(contexts::MOVEMENT)?,
            fly: c(contexts::FLY)?,
            text_entry: c(contexts::TEXT_ENTRY)?,
            menu: a("menu")?,
            screenshot: a("screenshot")?,
            toggle_fly: a("toggle_fly")?,
            toggle_debug: a("toggle_debug")?,
            open_chat: a("open_chat")?,
            mouselook: a("mouselook")?,
            look_x: a("look_x")?,
            look_y: a("look_y")?,
            zoom: a("zoom")?,
            reset_camera: a("reset_camera")?,
            move_forward: a("move_forward")?,
            strafe: a("strafe")?,
            turn: a("turn")?,
            jump: a("jump")?,
            walk: a("walk")?,
            autorun: a("autorun")?,
            fly_vertical: a("fly_vertical")?,
            fly_boost: a("fly_boost")?,
            fly_slow: a("fly_slow")?,
            fly_speed: a("fly_speed")?,
            text_submit: a("text_submit")?,
            text_cancel: a("text_cancel")?,
            text_backspace: a("text_backspace")?,
            text_history_prev: a("text_history_prev")?,
            text_history_next: a("text_history_next")?,
        })
    }

    /// Movement intent for this frame. `autorun` adds full forward input
    /// unless the player is already pulling backwards. Mouse look while
    /// `mouselook` is held turns the body as well (the caller decides
    /// whether to also feed [`StandardActions::look`] into the camera).
    #[must_use]
    pub fn intent(&self, m: &InputMapper, autorun: bool) -> MoveIntent {
        let mut forward = m.value(self.move_forward);
        if autorun && forward >= 0.0 {
            forward = 1.0;
        }
        MoveIntent {
            forward,
            strafe: m.value(self.strafe),
            turn: m.value(self.turn),
            vertical: m.value(self.fly_vertical),
            jump: m.just_pressed(self.jump),
            walk: m.pressed(self.walk),
            boost: m.pressed(self.fly_boost),
            slow: m.pressed(self.fly_slow),
        }
        .sanitized()
    }

    /// Look change `(yaw, pitch)` in radians for a frame of `dt` seconds:
    /// mouse deltas only while `mouselook` is held or `captured` is true
    /// (cursor captured by the caller), plus stick deflection at
    /// `stick_rate`. Positive yaw turns left (counter-clockwise seen from
    /// above, `+Z` up); positive pitch looks up.
    ///
    /// `look_x`/`look_y` follow the mouse convention (`+` = right / down),
    /// so both the mouse and the stick term are subtracted: the default
    /// `PadRightY` binding has scale −1 (stick pushed away = `look_y` −1 =
    /// look up, like moving the mouse away).
    #[must_use]
    pub fn look(
        &self,
        m: &InputMapper,
        settings: &LookSettings,
        dt: f32,
        captured: bool,
    ) -> (f32, f32) {
        let dt = if dt.is_finite() {
            dt.clamp(0.0, 0.25)
        } else {
            0.0
        };
        let mouse = captured || m.pressed(self.mouselook);
        let (mx, my) = if mouse {
            (m.delta(self.look_x), m.delta(self.look_y))
        } else {
            (0.0, 0.0)
        };
        let sens = settings.mouse_sensitivity;
        let sy = if settings.invert_y { -1.0 } else { 1.0 };
        // Mouse +x is right (turn right = negative yaw); +y is down.
        let yaw = -mx * sens - m.value(self.look_x) * settings.stick_rate * dt;
        let pitch = sy * (-my * sens - m.value(self.look_y) * settings.stick_rate * dt);
        let f = |v: f32| {
            if v.is_finite() {
                v.clamp(-10.0, 10.0)
            } else {
                0.0
            }
        };
        (f(yaw), f(pitch))
    }

    /// Activate the contexts for walking (global, camera, movement).
    pub fn enter_walk_mode(&self, m: &mut InputMapper) {
        m.deactivate(self.fly);
        m.activate(self.global);
        m.activate(self.camera);
        m.activate(self.movement);
    }

    /// Activate the contexts for free flight (fly above movement).
    pub fn enter_fly_mode(&self, m: &mut InputMapper) {
        m.activate(self.global);
        m.activate(self.camera);
        m.activate(self.movement);
        m.activate(self.fly);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::device::{KeyCode, MouseButton};
    use crate::mapper::RawEvent;

    #[test]
    fn defaults_compile_cleanly() {
        let reg = default_registry();
        assert_eq!(reg.action_count(), action_defs().len());
        let km = default_keymap();
        // Every built-in action is bound and every entry parsed.
        for (c, a, entries) in default_table() {
            assert_eq!(km.get(c, a).unwrap().len(), entries.len(), "{c}.{a}");
        }
        for (_, def) in reg.actions() {
            assert!(
                km.iter().any(|(_, a, _)| a == def.name),
                "{} unbound",
                def.name
            );
        }
        let (_, w) = InputMapper::new(reg.clone(), &km);
        assert!(w.is_empty(), "{w:?}");
        assert!(StandardActions::new(&reg).is_some());
        assert!(StandardActions::new(&Registry::new()).is_none());
    }

    fn walk_mapper() -> (StandardActions, InputMapper) {
        let reg = default_registry();
        let ids = StandardActions::new(&reg).unwrap();
        let (mut m, _) = InputMapper::new(reg, &default_keymap());
        ids.enter_walk_mode(&mut m);
        (ids, m)
    }

    fn mouse(m: &mut InputMapper, button: MouseButton, pressed: bool) {
        m.handle(&RawEvent::MouseButton { button, pressed }, Duration::ZERO);
    }

    #[test]
    fn stick_and_mouse_pitch_agree() {
        let (ids, mut m) = walk_mapper();
        let t = Duration::ZERO;
        let s = LookSettings::default();
        // Mouse moved away from the player (up) with mouse look held.
        mouse(&mut m, MouseButton::Right, true);
        m.handle(
            &RawEvent::MouseMotion {
                dx: 0.0,
                dy: -100.0,
            },
            t,
        );
        m.update(t);
        let (_, mouse_pitch) = ids.look(&m, &s, 0.1, false);
        mouse(&mut m, MouseButton::Right, false);
        // Right stick pushed away from the player (§4.3: +Y).
        m.handle(
            &RawEvent::PadAxis {
                axis: AnalogAxis::PadRightY,
                value: 1.0,
            },
            t,
        );
        m.update(t);
        let (_, stick_pitch) = ids.look(&m, &s, 0.1, false);
        assert!(
            mouse_pitch > 0.0 && stick_pitch > 0.0,
            "{mouse_pitch} {stick_pitch}"
        );
        // Inverting flips both.
        let inv = LookSettings {
            invert_y: true,
            ..s
        };
        assert!(ids.look(&m, &inv, 0.1, false).1 < 0.0);
        // Stick right turns right (negative yaw), like the mouse.
        m.handle(
            &RawEvent::PadAxis {
                axis: AnalogAxis::PadRightX,
                value: 1.0,
            },
            t,
        );
        m.update(t);
        assert!(ids.look(&m, &s, 0.1, false).0 < 0.0);
    }

    #[test]
    fn mouselook_survives_releasing_left_in_either_order() {
        for order in [
            [MouseButton::Left, MouseButton::Right],
            [MouseButton::Right, MouseButton::Left],
        ] {
            let (ids, mut m) = walk_mapper();
            for b in order {
                mouse(&mut m, b, true);
            }
            m.update(Duration::ZERO);
            assert!(m.pressed(ids.mouselook), "{order:?}");
            assert_eq!(m.value(ids.move_forward), 1.0, "{order:?}");
            mouse(&mut m, MouseButton::Left, false);
            m.update(Duration::ZERO);
            assert!(m.pressed(ids.mouselook), "{order:?}: right still held");
            assert_eq!(m.value(ids.move_forward), 0.0, "{order:?}");
            mouse(&mut m, MouseButton::Right, false);
            m.update(Duration::ZERO);
            assert!(!m.pressed(ids.mouselook), "{order:?}");
        }
    }

    #[test]
    fn walk_and_fly_mode_bindings() {
        let reg = default_registry();
        let ids = StandardActions::new(&reg).unwrap();
        let (mut m, _) = InputMapper::new(reg, &default_keymap());
        ids.enter_walk_mode(&mut m);
        let t = Duration::ZERO;
        let k = |key, pressed| RawEvent::Key {
            key,
            pressed,
            repeat: false,
        };
        m.handle(&k(KeyCode::Space, true), t);
        m.handle(&k(KeyCode::ShiftLeft, true), t);
        m.handle(&k(KeyCode::W, true), t);
        m.update(t);
        let i = ids.intent(&m, false);
        assert!(i.jump && i.walk && i.forward == 1.0 && i.vertical == 0.0);
        m.handle(&k(KeyCode::Space, false), t);
        ids.enter_fly_mode(&mut m);
        m.handle(&k(KeyCode::Space, true), t);
        m.update(t);
        let i = ids.intent(&m, false);
        assert!(!i.jump && i.vertical == 1.0 && i.boost);
        // Mouse look only while the right button is held.
        m.handle(&RawEvent::MouseMotion { dx: 100.0, dy: 0.0 }, t);
        m.update(t);
        assert_eq!(
            ids.look(&m, &LookSettings::default(), 0.016, false),
            (0.0, 0.0)
        );
        m.handle(
            &RawEvent::MouseButton {
                button: MouseButton::Right,
                pressed: true,
            },
            t,
        );
        m.handle(
            &RawEvent::MouseMotion {
                dx: 100.0,
                dy: -40.0,
            },
            t,
        );
        m.update(t);
        let (yaw, pitch) = ids.look(&m, &LookSettings::default(), 0.016, false);
        assert!(
            (yaw + 0.25).abs() < 1e-6 && (pitch - 0.1).abs() < 1e-6,
            "{yaw} {pitch}"
        );
        // Both buttons: run forward and keep looking, in either order.
        m.handle(
            &RawEvent::MouseButton {
                button: MouseButton::Right,
                pressed: false,
            },
            t,
        );
        m.handle(&k(KeyCode::W, false), t);
        for b in [MouseButton::Left, MouseButton::Right] {
            m.handle(
                &RawEvent::MouseButton {
                    button: b,
                    pressed: true,
                },
                t,
            );
        }
        m.handle(&RawEvent::MouseMotion { dx: 10.0, dy: 0.0 }, t);
        m.update(t);
        assert_eq!(ids.intent(&m, false).forward, 1.0);
        assert!(ids.look(&m, &LookSettings::default(), 0.016, false).0 < 0.0);
        for b in [MouseButton::Left, MouseButton::Right] {
            m.handle(
                &RawEvent::MouseButton {
                    button: b,
                    pressed: false,
                },
                t,
            );
        }
        // Autorun.
        m.handle(&k(KeyCode::W, false), t);
        m.update(t);
        assert_eq!(ids.intent(&m, true).forward, 1.0);
    }
}
