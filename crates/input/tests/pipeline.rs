// SPDX-License-Identifier: GPL-3.0-or-later
//! End-to-end: raw events → mapper → intent → fixed step → controllers, on
//! a synthetic height field.

use std::time::Duration;

use glam::Vec3;
use tessera_input::{
    BindingSpec, Chord, FixedTimestep, FlyConfig, FlyController, GroundState, InputMapper,
    IntentLatch, KeyCode, Keymap, LookSettings, RawEvent, StandardActions, WalkConfig,
    WalkController, default_keymap, default_registry,
};

fn key(key: KeyCode, pressed: bool) -> RawEvent {
    RawEvent::Key {
        key,
        pressed,
        repeat: false,
    }
}

/// Rolling hills: walkable everywhere (max slope ≈ 31°).
fn hills(x: f32, y: f32) -> Option<f32> {
    Some((x * 0.2).sin() * 3.0 + (y * 0.15).cos() * 2.0)
}

#[test]
fn walk_across_hills_with_user_overrides() {
    let registry = default_registry();
    let ids = StandardActions::new(&registry).unwrap();
    // The user rebinds jump to J and forward to I, on top of the defaults.
    let user = Keymap::parse(
        "[bindings.movement]\njump = [\"J\"]\nmove_forward = [\"I\", { input = \"K\", value = -1.0 }]\n",
    )
    .unwrap();
    let keymap = default_keymap().overlay(&user);
    let forward = keymap.get("movement", "move_forward").unwrap();
    let w = Chord::parse("W").unwrap();
    assert!(
        !forward
            .iter()
            .any(|b| matches!(b, BindingSpec::Digital { chord, .. } if *chord == w)),
        "{forward:?}"
    );
    let (mut input, warnings) = InputMapper::new(registry, &keymap);
    assert!(warnings.is_empty(), "{warnings:?}");
    ids.enter_walk_mode(&mut input);

    let mut walker = WalkController::new(Vec3::new(0.0, 0.0, 10.0), 0.0, WalkConfig::default());
    let mut step = FixedTimestep::default();
    let frame = 1.0 / 60.0;
    let mut now = Duration::ZERO;
    let mut latch = IntentLatch::default();
    let mut jumped = false;
    let mut landed_after_jump = false;
    for f in 0..600 {
        now += Duration::from_secs_f64(frame);
        match f {
            // The old binding does nothing any more: W alone (I not held)
            // gives no forward value and the walker stays put.
            5 => input.handle(&key(KeyCode::W, true), now),
            25 => input.handle(&key(KeyCode::W, false), now),
            30 => input.handle(&key(KeyCode::I, true), now),
            200 => input.handle(&key(KeyCode::J, true), now),
            201 => input.handle(&key(KeyCode::J, false), now),
            _ => {}
        }
        input.update(now);
        if (5..25).contains(&f) {
            assert_eq!(input.value(ids.move_forward), 0.0, "frame {f}");
            assert!(!input.pressed(ids.move_forward), "frame {f}");
        }
        if f == 29 {
            let p = walker.position();
            assert!(p.x.abs() < 1e-4 && p.y.abs() < 1e-4, "moved on W: {p}");
        }
        latch.frame(ids.intent(&input, false));
        for _ in 0..step.advance(frame) {
            let ev = walker.tick(&latch.tick(), &hills, step.dt() as f32);
            jumped |= ev.jumped;
            if jumped && ev.landed.is_some() {
                landed_after_jump = true;
            }
        }
        let p = walker.interpolated(step.alpha());
        assert!(p.is_finite());
    }
    assert!(jumped && landed_after_jump);
    let p = walker.position();
    // ~9.5 s of running at 6 u/s along +X.
    assert!(p.x > 50.0 && p.x < 60.0, "{p}");
    assert_eq!(walker.state(), GroundState::Grounded);
    assert!((p.z - hills(p.x, p.y).unwrap()).abs() < 1e-3);
}

#[test]
fn fly_mode_toggle_and_look() {
    let registry = default_registry();
    let ids = StandardActions::new(&registry).unwrap();
    let (mut input, _) = InputMapper::new(registry, &default_keymap());
    ids.enter_walk_mode(&mut input);
    let mut fly = FlyController::new(Vec3::ZERO, 0.0, 0.0, FlyConfig::default());
    let mut flying = false;
    let mut step = FixedTimestep::default();
    let mut now = Duration::ZERO;
    let script: Vec<(usize, RawEvent)> = vec![
        (1, key(KeyCode::ControlLeft, true)),
        (1, key(KeyCode::F, true)),
        (2, key(KeyCode::F, false)),
        (2, key(KeyCode::ControlLeft, false)),
        (3, key(KeyCode::Space, true)),
        (
            3,
            RawEvent::MouseButton {
                button: tessera_input::MouseButton::Right,
                pressed: true,
            },
        ),
        (
            4,
            RawEvent::MouseMotion {
                dx: -200.0,
                dy: 0.0,
            },
        ),
    ];
    for f in 0..60 {
        now += Duration::from_millis(16);
        for (_, e) in script.iter().filter(|(at, _)| *at == f) {
            input.handle(e, now);
        }
        input.update(now);
        if input.just_pressed(ids.toggle_fly) {
            flying = !flying;
            if flying {
                ids.enter_fly_mode(&mut input);
            } else {
                ids.enter_walk_mode(&mut input);
            }
        }
        let (dyaw, dpitch) = ids.look(&input, &LookSettings::default(), 0.016, false);
        fly.look(dyaw, dpitch);
        let intent = ids.intent(&input, false);
        for _ in 0..step.advance(0.016) {
            fly.tick(&intent, step.dt() as f32);
        }
    }
    assert!(flying);
    // Space rises in fly mode; the mouse turned the view left by 0.5 rad.
    assert!(fly.position().z > 10.0, "{}", fly.position());
    assert!((fly.yaw() - 0.5).abs() < 1e-5, "{}", fly.yaw());
}
