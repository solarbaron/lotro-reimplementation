// SPDX-License-Identifier: GPL-3.0-or-later
//! Unit tests of the mapper's game-band behaviour (kept from before decision
//! 0014; the UI bridge is tested in `tests/ui_bridge.rs`).

use super::*;
use crate::keymap::BindingSpec;

const T0: Duration = Duration::ZERO;

fn key(k: KeyCode, pressed: bool) -> RawEvent {
    RawEvent::Key {
        key: k,
        pressed,
        repeat: false,
    }
}

struct Fixture {
    m: InputMapper,
    jump: ActionId,
    fwd: ActionId,
    save: ActionId,
    seq: ActionId,
    look: ActionId,
    chat: ActionId,
    sprint: ActionId,
    global: ContextId,
    movement: ContextId,
    text: ContextId,
}

fn fixture(edit: impl FnOnce(&mut Keymap)) -> Fixture {
    let mut r = Registry::new();
    let jump = r.add_action(ActionDef::button("jump")).unwrap();
    let fwd = r.add_action(ActionDef::axis("forward")).unwrap();
    let save = r.add_action(ActionDef::button("save")).unwrap();
    let seq = r.add_action(ActionDef::button("seq")).unwrap();
    let look = r.add_action(ActionDef::axis("look")).unwrap();
    let chat = r.add_action(ActionDef::button("chat")).unwrap();
    let sprint = r
        .add_action(ActionDef::button("sprint").with_modifiers(ModifierPolicy::Subset))
        .unwrap();
    let global = r.add_context(ContextDef::new("global", 0)).unwrap();
    let movement = r.add_context(ContextDef::new("movement", 10)).unwrap();
    let text = r
        .add_context(ContextDef::new("text", 100).opaque().with_text())
        .unwrap();
    let doc = r#"
[bindings.global]
save = ["Ctrl+S"]
seq = ["G G", "Ctrl+K Ctrl+C"]
chat = ["Enter"]
[bindings.movement]
jump = ["Space", "WheelDown"]
forward = ["W", { input = "S", value = -1 }, "MouseLeft+MouseRight", { axis = "PadLeftY", deadzone = 0.2 }]
look = [{ axis = "MouseX", scale = 0.5 }]
sprint = ["Shift"]
"#;
    let mut km = Keymap::parse(doc).unwrap();
    edit(&mut km);
    let (mut m, w) = InputMapper::new(r, &km);
    assert!(w.is_empty(), "{w:?}");
    m.activate(global);
    m.activate(movement);
    Fixture {
        m,
        jump,
        fwd,
        save,
        seq,
        look,
        chat,
        sprint,
        global,
        movement,
        text,
    }
}

#[test]
fn press_hold_release_edges() {
    let mut f = fixture(|_| {});
    f.m.handle(&key(KeyCode::Space, true), T0);
    f.m.update(T0);
    let s = f.m.state(f.jump);
    assert!(s.pressed && s.just_pressed() && !s.just_released());
    assert_eq!(s.value, 1.0);
    // Auto-repeat changes nothing.
    f.m.handle(
        &RawEvent::Key {
            key: KeyCode::Space,
            pressed: true,
            repeat: true,
        },
        T0,
    );
    f.m.update(T0);
    let s = f.m.state(f.jump);
    assert!(s.pressed && !s.just_pressed());
    f.m.handle(&key(KeyCode::Space, false), T0);
    f.m.update(T0);
    let s = f.m.state(f.jump);
    assert!(!s.pressed && s.just_released());
    // Press and release inside one frame keeps both edges.
    f.m.handle(&key(KeyCode::Space, true), T0);
    f.m.handle(&key(KeyCode::Space, false), T0);
    f.m.update(T0);
    let s = f.m.state(f.jump);
    assert!(!s.pressed && s.presses == 1 && s.releases == 1);
}

#[test]
fn axis_from_keys_and_stick() {
    let mut f = fixture(|_| {});
    f.m.handle(&key(KeyCode::W, true), T0);
    f.m.update(T0);
    assert_eq!(f.m.value(f.fwd), 1.0);
    f.m.handle(&key(KeyCode::S, true), T0);
    f.m.update(T0);
    assert_eq!(f.m.value(f.fwd), 0.0);
    f.m.handle(&key(KeyCode::W, false), T0);
    f.m.update(T0);
    assert_eq!(f.m.value(f.fwd), -1.0);
    f.m.handle(&key(KeyCode::S, false), T0);
    f.m.handle(
        &RawEvent::PadAxis {
            axis: AnalogAxis::PadLeftY,
            value: 0.6,
        },
        T0,
    );
    f.m.update(T0);
    assert!((f.m.value(f.fwd) - 0.5).abs() < 1e-6);
    f.m.handle(
        &RawEvent::PadAxis {
            axis: AnalogAxis::PadLeftY,
            value: 0.1,
        },
        T0,
    );
    f.m.update(T0);
    assert_eq!(f.m.value(f.fwd), 0.0, "inside the dead zone");
    f.m.handle(
        &RawEvent::PadAxis {
            axis: AnalogAxis::PadLeftY,
            value: f32::NAN,
        },
        T0,
    );
    f.m.update(T0);
    assert_eq!(f.m.value(f.fwd), 0.0);
    // §7.5: a non-finite position is ignored; the previous one stays.
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        f.m.handle(
            &RawEvent::PadAxis {
                axis: AnalogAxis::PadLeftY,
                value: 0.6,
            },
            T0,
        );
        f.m.handle(
            &RawEvent::PadAxis {
                axis: AnalogAxis::PadLeftY,
                value: bad,
            },
            T0,
        );
        f.m.update(T0);
        assert!((f.m.value(f.fwd) - 0.5).abs() < 1e-6, "{bad}");
    }
}

#[test]
fn relative_axis_delta() {
    let mut f = fixture(|_| {});
    f.m.handle(&RawEvent::MouseMotion { dx: 10.0, dy: 3.0 }, T0);
    f.m.handle(&RawEvent::MouseMotion { dx: 4.0, dy: 3.0 }, T0);
    f.m.handle(
        &RawEvent::MouseMotion {
            dx: f64::INFINITY,
            dy: 0.0,
        },
        T0,
    );
    f.m.update(T0);
    assert_eq!(f.m.delta(f.look), 7.0);
    assert_eq!(f.m.value(f.look), 0.0);
    f.m.update(T0);
    assert_eq!(f.m.delta(f.look), 0.0, "deltas last one frame");
}

#[test]
fn modifiers_exact_vs_subset() {
    let mut f = fixture(|_| {});
    // Ctrl+S: the exact match in the lower `global` context wins over
    // movement's subset match of `S`.
    f.m.handle(&key(KeyCode::ControlLeft, true), T0);
    f.m.handle(&key(KeyCode::S, true), T0);
    f.m.update(T0);
    assert!(f.m.just_pressed(f.save));
    assert_eq!(f.m.value(f.fwd), 0.0);
    f.m.handle(&key(KeyCode::S, false), T0);
    // Alt+S: no exact match anywhere, so the subset match moves back.
    f.m.handle(&key(KeyCode::ControlLeft, false), T0);
    f.m.handle(&key(KeyCode::AltLeft, true), T0);
    f.m.handle(&key(KeyCode::S, true), T0);
    f.m.update(T0);
    assert_eq!(f.m.value(f.fwd), -1.0);
    f.m.handle(&key(KeyCode::S, false), T0);
    f.m.handle(&key(KeyCode::AltLeft, false), T0);
    f.m.handle(&key(KeyCode::ControlLeft, true), T0);
    // Exact policy: Shift+Ctrl+S does not fire save.
    f.m.handle(&key(KeyCode::S, false), T0);
    f.m.handle(&key(KeyCode::ShiftLeft, true), T0);
    f.m.handle(&key(KeyCode::S, true), T0);
    f.m.update(T0);
    assert!(!f.m.just_pressed(f.save));
}

#[test]
fn subset_movement_with_shift_and_modifier_hold() {
    let mut f = fixture(|_| {});
    f.m.handle(&key(KeyCode::ShiftLeft, true), T0);
    f.m.handle(&key(KeyCode::W, true), T0);
    f.m.update(T0);
    assert_eq!(f.m.value(f.fwd), 1.0);
    assert!(f.m.state(f.sprint).just_pressed());
    f.m.handle(&key(KeyCode::ShiftLeft, false), T0);
    f.m.update(T0);
    assert!(f.m.state(f.sprint).just_released());
    assert_eq!(f.m.value(f.fwd), 1.0, "W keeps moving");
    // The window system's modifier report works too.
    f.m.handle(&RawEvent::Modifiers(Modifiers::SHIFT), T0);
    f.m.update(T0);
    assert!(f.m.pressed(f.sprint));
}

#[test]
fn mouse_chord_any_order() {
    for order in [
        [MouseButton::Left, MouseButton::Right],
        [MouseButton::Right, MouseButton::Left],
    ] {
        let mut f = fixture(|_| {});
        for b in order {
            f.m.handle(
                &RawEvent::MouseButton {
                    button: b,
                    pressed: true,
                },
                T0,
            );
        }
        f.m.update(T0);
        assert_eq!(f.m.value(f.fwd), 1.0);
        f.m.handle(
            &RawEvent::MouseButton {
                button: MouseButton::Left,
                pressed: false,
            },
            T0,
        );
        f.m.update(T0);
        assert_eq!(f.m.value(f.fwd), 0.0);
    }
}

#[test]
fn sequences_and_timeout() {
    let mut f = fixture(|_| {});
    let t = |ms| Duration::from_millis(ms);
    f.m.handle(&key(KeyCode::G, true), t(0));
    f.m.handle(&key(KeyCode::G, false), t(10));
    assert!(f.m.sequence_pending());
    f.m.handle(&key(KeyCode::G, true), t(200));
    f.m.update(t(210));
    assert!(f.m.just_pressed(f.seq));
    f.m.handle(&key(KeyCode::G, false), t(220));
    f.m.update(t(230));
    assert!(!f.m.pressed(f.seq));
    // Timeout.
    f.m.handle(&key(KeyCode::G, true), t(1000));
    f.m.handle(&key(KeyCode::G, false), t(1010));
    f.m.update(t(2500));
    assert!(!f.m.sequence_pending());
    f.m.handle(&key(KeyCode::G, true), t(2600));
    f.m.update(t(2610));
    assert!(!f.m.just_pressed(f.seq));
    assert!(f.m.sequence_pending());
    f.m.handle(&key(KeyCode::G, false), t(2620));
    // A wrong second stroke breaks the sequence and is handled normally.
    f.m.handle(&key(KeyCode::Space, true), t(2700));
    f.m.update(t(2710));
    assert!(!f.m.sequence_pending());
    assert!(f.m.just_pressed(f.jump));
    // Modifier chords.
    f.m.handle(&key(KeyCode::ControlLeft, true), t(3000));
    f.m.handle(&key(KeyCode::K, true), t(3010));
    f.m.handle(&key(KeyCode::K, false), t(3020));
    f.m.handle(&key(KeyCode::C, true), t(3030));
    f.m.update(t(3040));
    assert!(f.m.just_pressed(f.seq));
}

#[test]
fn sequence_survives_modifier_presses_between_strokes() {
    let t = |ms| Duration::from_millis(ms);
    // Ctrl released and pressed again between the strokes.
    let mut f = fixture(|_| {});
    f.m.handle(&key(KeyCode::ControlLeft, true), t(0));
    f.m.handle(&key(KeyCode::K, true), t(10));
    f.m.handle(&key(KeyCode::K, false), t(20));
    f.m.handle(&key(KeyCode::ControlLeft, false), t(30));
    assert!(f.m.sequence_pending());
    f.m.handle(&key(KeyCode::ControlLeft, true), t(40));
    assert!(f.m.sequence_pending(), "a modifier press does not break it");
    f.m.handle(&key(KeyCode::C, true), t(50));
    f.m.update(t(60));
    assert!(f.m.just_pressed(f.seq));
    // A different modifier on the second stroke.
    let mut f = fixture(|k| {
        k.set(
            "global",
            "chat",
            vec![BindingSpec::parse("Ctrl+K Shift+C").unwrap()],
        )
        .unwrap();
    });
    f.m.handle(&key(KeyCode::ControlLeft, true), t(0));
    f.m.handle(&key(KeyCode::K, true), t(10));
    f.m.handle(&key(KeyCode::K, false), t(20));
    f.m.handle(&key(KeyCode::ControlLeft, false), t(30));
    f.m.handle(&key(KeyCode::ShiftLeft, true), t(40));
    f.m.handle(&key(KeyCode::C, true), t(50));
    f.m.update(t(60));
    assert!(f.m.just_pressed(f.chat));
    assert!(!f.m.just_pressed(f.seq));
    // The wrong modifier still breaks it when the button comes.
    let mut f = fixture(|_| {});
    f.m.handle(&key(KeyCode::ControlLeft, true), t(0));
    f.m.handle(&key(KeyCode::K, true), t(10));
    f.m.handle(&key(KeyCode::K, false), t(20));
    f.m.handle(&key(KeyCode::ControlLeft, false), t(30));
    f.m.handle(&key(KeyCode::AltLeft, true), t(40));
    f.m.handle(&key(KeyCode::C, true), t(50));
    f.m.update(t(60));
    assert!(!f.m.just_pressed(f.seq) && !f.m.sequence_pending());
    // The timeout still applies.
    let mut f = fixture(|_| {});
    f.m.handle(&key(KeyCode::ControlLeft, true), t(0));
    f.m.handle(&key(KeyCode::K, true), t(10));
    f.m.handle(&key(KeyCode::ControlLeft, false), t(20));
    f.m.handle(&key(KeyCode::ControlLeft, true), t(2000));
    assert!(!f.m.sequence_pending());
}

#[test]
fn sequence_waits_for_the_rest_of_a_chord_stroke() {
    let t = |ms| Duration::from_millis(ms);
    let mut f = fixture(|k| {
        k.set(
            "global",
            "chat",
            vec![BindingSpec::parse("G MouseLeft+MouseRight").unwrap()],
        )
        .unwrap();
    });
    f.m.handle(&key(KeyCode::G, true), t(0));
    f.m.handle(&key(KeyCode::G, false), t(10));
    let m = |button, pressed| RawEvent::MouseButton { button, pressed };
    f.m.handle(&m(MouseButton::Left, true), t(20));
    assert!(f.m.sequence_pending());
    f.m.handle(&m(MouseButton::Right, true), t(30));
    f.m.update(t(40));
    assert!(f.m.just_pressed(f.chat));
}

#[test]
fn register_action_at_run_time_keeps_state() {
    let mut f = fixture(|_| {});
    let mut km = f.m.keymap().clone();
    km.set(
        "movement",
        "plug.toggle",
        vec![BindingSpec::parse("T").unwrap()],
    )
    .unwrap();
    let w = f.m.set_keymap(&km);
    assert!(matches!(
        w.as_slice(),
        [KeymapWarning::UnknownAction { .. }]
    ));
    assert!(f.m.registry().action("plug.toggle").is_none());
    // W held across the registration keeps moving, without edges.
    f.m.handle(&key(KeyCode::W, true), T0);
    f.m.update(T0);
    let toggle =
        f.m.register_action(ActionDef::button("plug.toggle"))
            .unwrap();
    f.m.update(T0);
    assert_eq!(f.m.value(f.fwd), 1.0);
    assert!(!f.m.state(f.fwd).just_released());
    f.m.handle(&key(KeyCode::T, true), T0);
    f.m.update(T0);
    assert!(f.m.just_pressed(toggle));
    f.m.handle(&key(KeyCode::W, false), T0);
    f.m.update(T0);
    assert!(f.m.state(f.fwd).just_released());
    assert!(
        f.m.register_action(ActionDef::button("plug.toggle"))
            .is_err()
    );
    // A late context is inactive until activated.
    let ctx = f.m.register_context(ContextDef::new("plug", 50)).unwrap();
    assert!(!f.m.is_active(ctx));
    f.m.activate(ctx);
    assert_eq!(f.m.ordered()[0], ctx);
}

#[test]
fn compile_warnings_are_capped() {
    let mut km = Keymap::new();
    for i in 0..500 {
        km.set("nowhere", &format!("a{i}"), vec![]).unwrap();
        km.set("global", &format!("unknown{i}"), vec![]).unwrap();
    }
    let (_, w) = InputMapper::new(Registry::new(), &km);
    assert_eq!(w.len(), 2, "one per unknown context");
    let mut r = Registry::new();
    r.add_context(ContextDef::new("global", 0)).unwrap();
    let (_, w) = InputMapper::new(r, &km);
    assert_eq!(w.len(), crate::MAX_KEYMAP_WARNINGS + 1);
}

#[test]
fn prefix_conflict_disables_sequence() {
    let mut r = Registry::new();
    r.add_action(ActionDef::button("a")).unwrap();
    r.add_action(ActionDef::button("b")).unwrap();
    r.add_context(ContextDef::new("c", 0)).unwrap();
    let km = Keymap::parse("[bindings.c]\na = [\"G\"]\nb = [\"G H\", \"X\"]\n").unwrap();
    let (_, w) = InputMapper::new(r, &km);
    assert!(
        matches!(w.as_slice(), [KeymapWarning::PrefixConflict { .. }]),
        "{w:?}"
    );
}

#[test]
fn context_priority_and_opaque() {
    let mut f = fixture(|k| {
        k.set("text", "chat", vec![BindingSpec::parse("Space").unwrap()])
            .unwrap();
    });
    // Enter is bound only in global; it falls through movement.
    f.m.handle(&key(KeyCode::Enter, true), T0);
    f.m.update(T0);
    assert!(f.m.just_pressed(f.chat));
    f.m.handle(&key(KeyCode::Enter, false), T0);
    // Hold W, then open the opaque text context: W is released.
    f.m.handle(&key(KeyCode::W, true), T0);
    f.m.update(T0);
    assert_eq!(f.m.value(f.fwd), 1.0);
    f.m.activate(f.text);
    f.m.update(T0);
    assert_eq!(f.m.value(f.fwd), 0.0);
    // Space goes to the text context's binding, not jump.
    f.m.handle(&key(KeyCode::Space, true), T0);
    f.m.update(T0);
    assert!(f.m.just_pressed(f.chat));
    assert!(!f.m.pressed(f.jump));
    f.m.handle(&key(KeyCode::Space, false), T0);
    // Unbound keys are swallowed: Enter no longer reaches global.
    f.m.handle(&key(KeyCode::Enter, true), T0);
    f.m.update(T0);
    assert!(!f.m.just_pressed(f.chat));
    f.m.handle(&key(KeyCode::Enter, false), T0);
    // Mouse motion is swallowed too.
    f.m.handle(&RawEvent::MouseMotion { dx: 5.0, dy: 0.0 }, T0);
    f.m.handle(&RawEvent::Text('h'), T0);
    f.m.handle(&RawEvent::Text('\u{7}'), T0);
    f.m.update(T0);
    assert_eq!(f.m.delta(f.look), 0.0);
    assert_eq!(f.m.text(), "h");
    f.m.deactivate(f.text);
    f.m.handle(&RawEvent::Text('x'), T0);
    f.m.update(T0);
    assert_eq!(f.m.text(), "");
    assert_eq!(f.m.ordered(), vec![f.movement, f.global]);
}

#[test]
fn equal_priority_most_recent_wins_and_deactivate_releases() {
    let mut r = Registry::new();
    let a = r.add_action(ActionDef::button("a")).unwrap();
    let b = r.add_action(ActionDef::button("b")).unwrap();
    let c1 = r.add_context(ContextDef::new("one", 5)).unwrap();
    let c2 = r.add_context(ContextDef::new("two", 5)).unwrap();
    let km = Keymap::parse("[bindings.one]\na = [\"X\"]\n[bindings.two]\nb = [\"X\"]\n").unwrap();
    let (mut m, _) = InputMapper::new(r, &km);
    m.activate(c1);
    m.activate(c2);
    m.handle(&key(KeyCode::X, true), T0);
    m.update(T0);
    assert!(m.pressed(b) && !m.pressed(a));
    m.handle(&key(KeyCode::X, false), T0);
    m.activate(c1); // re-activation moves it to the top
    m.handle(&key(KeyCode::X, true), T0);
    m.update(T0);
    assert!(m.pressed(a) && !m.pressed(b));
    m.deactivate(c1);
    m.update(T0);
    assert!(m.state(a).just_released());
    // X is still physically held; it does not re-trigger in `two`.
    assert!(!m.pressed(b));
}

#[test]
fn wheel_clicks_are_momentary() {
    let mut f = fixture(|_| {});
    f.m.handle(&RawEvent::Wheel { dx: 0.0, dy: -0.5 }, T0);
    f.m.update(T0);
    assert!(!f.m.just_pressed(f.jump));
    f.m.handle(&RawEvent::Wheel { dx: 0.0, dy: -0.6 }, T0);
    f.m.update(T0);
    let s = f.m.state(f.jump);
    assert!(s.just_pressed() && s.just_released() && !s.pressed);
    f.m.handle(&RawEvent::Wheel { dx: 0.0, dy: -1e30 }, T0);
    f.m.update(T0);
    assert_eq!(f.m.state(f.jump).presses, MAX_WHEEL_CLICKS_PER_EVENT as u16);
}

#[test]
fn focus_loss_releases_everything() {
    let mut f = fixture(|_| {});
    f.m.handle(&key(KeyCode::W, true), T0);
    f.m.handle(&key(KeyCode::ShiftLeft, true), T0);
    f.m.update(T0);
    f.m.handle(&RawEvent::FocusLost, T0);
    f.m.update(T0);
    assert_eq!(f.m.value(f.fwd), 0.0);
    assert!(!f.m.pressed(f.sprint));
    assert_eq!(f.m.mods(), Modifiers::empty());
    // The key's release after refocus is harmless.
    f.m.handle(&key(KeyCode::W, false), T0);
    f.m.update(T0);
    assert!(!f.m.state(f.fwd).just_released());
}

#[test]
fn analog_button_threshold_with_hysteresis() {
    let mut f = fixture(|k| {
        let spec = BindingSpec::analog(AnalogAxis::PadRightTrigger, 1.0, 0.0);
        k.set("global", "chat", vec![spec]).unwrap();
    });
    let set = |m: &mut InputMapper, v| {
        m.handle(
            &RawEvent::PadAxis {
                axis: AnalogAxis::PadRightTrigger,
                value: v,
            },
            T0,
        );
        m.update(T0);
    };
    set(&mut f.m, 0.45);
    assert!(!f.m.pressed(f.chat));
    set(&mut f.m, 0.55);
    assert!(f.m.just_pressed(f.chat));
    set(&mut f.m, 0.45);
    assert!(f.m.pressed(f.chat), "hysteresis keeps it pressed");
    set(&mut f.m, 0.3);
    assert!(f.m.state(f.chat).just_released());
}

/// keymap.md §2: an axis is pressed while a digital binding is held or
/// `|value| >= 0.5`; the 0.5/0.4 hysteresis of §7.5 is for buttons only.
#[test]
fn analog_axis_pressed_has_no_hysteresis() {
    let mut f = fixture(|_| {});
    let set = |m: &mut InputMapper, v| {
        m.handle(
            &RawEvent::PadAxis {
                axis: AnalogAxis::PadLeftY,
                value: v,
            },
            T0,
        );
        m.update(T0);
    };
    // Dead zone 0.2: 0.7 -> 0.625, 0.55 -> 0.4375.
    set(&mut f.m, 0.7);
    assert!(f.m.state(f.fwd).just_pressed());
    set(&mut f.m, 0.55);
    let s = f.m.state(f.fwd);
    assert!((s.value - 0.4375).abs() < 1e-6, "{s:?}");
    assert!(!s.pressed && s.just_released(), "{s:?}");
    set(&mut f.m, -0.7);
    assert!(f.m.state(f.fwd).just_pressed());
}

/// keymap.md §2, §5.1, §7.4: a modifier-only stroke is a held digital
/// binding, so its `value` counts towards an axis action.
#[test]
fn modifier_hold_adds_axis_value() {
    let mut f = fixture(|k| {
        let list = vec![
            BindingSpec::parse("W").unwrap(),
            BindingSpec::digital(crate::chord::Chord::parse("Ctrl").unwrap(), -1.0),
        ];
        k.set("movement", "forward", list).unwrap();
    });
    f.m.handle(&key(KeyCode::ControlLeft, true), T0);
    f.m.update(T0);
    let s = f.m.state(f.fwd);
    assert!(s.pressed && s.presses == 1, "{s:?}");
    assert_eq!(s.value, -1.0);
    // W (subset policy) adds +1 on top.
    f.m.handle(&key(KeyCode::W, true), T0);
    f.m.update(T0);
    assert_eq!(f.m.value(f.fwd), 0.0);
    f.m.handle(&key(KeyCode::ControlLeft, false), T0);
    f.m.update(T0);
    assert_eq!(f.m.value(f.fwd), 1.0);
    f.m.handle(&key(KeyCode::W, false), T0);
    f.m.update(T0);
    let s = f.m.state(f.fwd);
    assert!(!s.pressed && s.value == 0.0, "{s:?}");
    // The window system's modifier report counts as well.
    f.m.handle(&RawEvent::Modifiers(Modifiers::CTRL), T0);
    f.m.update(T0);
    assert_eq!(f.m.value(f.fwd), -1.0);
}

#[test]
fn modifier_key_as_plain_binding() {
    let mut f = fixture(|k| {
        k.set("global", "chat", vec![BindingSpec::parse("LCtrl").unwrap()])
            .unwrap();
    });
    f.m.handle(&key(KeyCode::ControlLeft, true), T0);
    f.m.update(T0);
    assert!(f.m.just_pressed(f.chat));
}

#[test]
fn keymap_reload_releases() {
    let mut f = fixture(|_| {});
    f.m.handle(&key(KeyCode::Space, true), T0);
    f.m.update(T0);
    let w = f.m.set_keymap(&Keymap::new());
    assert!(w.is_empty());
    f.m.update(T0);
    assert!(f.m.state(f.jump).just_released());
    let _ = (f.global, f.save);
}

#[test]
fn many_held_buttons_bounded() {
    let mut f = fixture(|_| {});
    for &k in KeyCode::ALL {
        f.m.handle(&key(k, true), T0);
    }
    for n in 6..=32 {
        f.m.handle(
            &RawEvent::MouseButton {
                button: MouseButton::Other(n),
                pressed: true,
            },
            T0,
        );
    }
    f.m.update(T0);
    assert!(f.m.held.len() <= MAX_HELD_BUTTONS);
}
