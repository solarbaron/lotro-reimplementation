// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-input` runtime (docs/specs/keymap.md §7, §9): an arbitrary stream of raw events,
//! context changes and frame updates through `InputMapper` with the built-in registry and
//! keymap, feeding the fly and walk controllers over a hostile height field. Nothing may
//! panic, hang or grow without bound, and controller state must stay finite.

#![no_main]

use std::time::Duration;

use libfuzzer_sys::fuzz_target;
use tessera_input::{
    AnalogAxis, FixedTimestep, FlyConfig, FlyController, InputMapper, IntentLatch, KeyCode,
    LookSettings, Modifiers, MouseButton, PadButton, RawEvent, StandardActions, WalkConfig,
    WalkController, default_keymap, default_registry,
};

struct Bytes<'a>(&'a [u8]);

impl Bytes<'_> {
    fn u8(&mut self) -> Option<u8> {
        let (&b, rest) = self.0.split_first()?;
        self.0 = rest;
        Some(b)
    }
    fn u32(&mut self) -> Option<u32> {
        let (b, rest) = self.0.split_first_chunk::<4>()?;
        self.0 = rest;
        Some(u32::from_le_bytes(*b))
    }
    fn f32(&mut self) -> Option<f32> {
        self.u32().map(f32::from_bits)
    }
    fn f64(&mut self) -> Option<f64> {
        let (b, rest) = self.0.split_first_chunk::<8>()?;
        self.0 = rest;
        Some(f64::from_bits(u64::from_le_bytes(*b)))
    }
}

fuzz_target!(|data: &[u8]| {
    let registry = default_registry();
    let ids = StandardActions::new(&registry).expect("built-in ids");
    let contexts = [
        ids.global,
        ids.camera,
        ids.movement,
        ids.fly,
        ids.text_entry,
    ];
    let (mut m, _) = InputMapper::new(registry, &default_keymap());
    ids.enter_walk_mode(&mut m);

    let mut heights = [0.0f32; 16];
    let mut walker =
        WalkController::new(tessera_input::glam::Vec3::ZERO, 0.0, WalkConfig::default());
    let mut fly = FlyController::new(
        tessera_input::glam::Vec3::ZERO,
        0.0,
        0.0,
        FlyConfig::default(),
    );
    let mut step = FixedTimestep::default();
    let mut latch = IntentLatch::default();
    let mut now = Duration::ZERO;
    let mut b = Bytes(data);
    let mut ops = 0;
    while let Some(op) = b.u8() {
        ops += 1;
        if ops > 4096 {
            break;
        }
        let ev = match op % 13 {
            0 => {
                let Some(x) = b.u8() else { break };
                let key = KeyCode::ALL[usize::from(x) % KeyCode::ALL.len()];
                Some(RawEvent::Key {
                    key,
                    pressed: op & 0x40 != 0,
                    repeat: op & 0x80 != 0,
                })
            }
            1 => {
                let Some(x) = b.u8() else { break };
                let button = match x % 6 {
                    0 => MouseButton::Left,
                    1 => MouseButton::Right,
                    2 => MouseButton::Middle,
                    3 => MouseButton::Back,
                    4 => MouseButton::Forward,
                    _ => MouseButton::Other(u16::from(x)),
                };
                Some(RawEvent::MouseButton {
                    button,
                    pressed: op & 0x40 != 0,
                })
            }
            2 => {
                let (Some(dx), Some(dy)) = (b.f64(), b.f64()) else {
                    break;
                };
                Some(RawEvent::MouseMotion { dx, dy })
            }
            3 => {
                let (Some(dx), Some(dy)) = (b.f32(), b.f32()) else {
                    break;
                };
                Some(RawEvent::Wheel { dx, dy })
            }
            4 => {
                let Some(x) = b.u8() else { break };
                Some(RawEvent::PadButton {
                    button: PadButton::ALL[usize::from(x) % PadButton::ALL.len()],
                    pressed: op & 0x40 != 0,
                })
            }
            5 => {
                let (Some(x), Some(value)) = (b.u8(), b.f32()) else {
                    break;
                };
                Some(RawEvent::PadAxis {
                    axis: AnalogAxis::ALL[usize::from(x) % AnalogAxis::ALL.len()],
                    value,
                })
            }
            6 => b
                .u8()
                .map(|x| RawEvent::Modifiers(Modifiers::from_bits_truncate(x))),
            7 => b
                .u32()
                .map(|x| RawEvent::Text(char::from_u32(x).unwrap_or('?'))),
            8 => Some(RawEvent::FocusLost),
            9 => {
                let Some(x) = b.u8() else { break };
                m.set_active(contexts[usize::from(x) % contexts.len()], op & 0x40 != 0);
                None
            }
            10 => {
                let (Some(x), Some(h)) = (b.u8(), b.f32()) else {
                    break;
                };
                heights[usize::from(x) % heights.len()] = h;
                None
            }
            11 => {
                let (Some(x), Some(y), Some(z)) = (b.f32(), b.f32(), b.f32()) else {
                    break;
                };
                walker.teleport(tessera_input::glam::Vec3::new(x, y, z));
                fly.teleport(tessera_input::glam::Vec3::new(x, y, z));
                None
            }
            _ => {
                // End of frame.
                let Some(x) = b.u32() else { break };
                let ms = u64::from(x % 2_000_000);
                now = now.saturating_add(Duration::from_millis(ms));
                m.update(now);
                let _ = m.text();
                let frame = ms as f64 / 1000.0;
                let (dyaw, dpitch) = ids.look(&m, &LookSettings::default(), frame as f32, false);
                fly.look(dyaw, dpitch);
                walker.turn(dyaw);
                latch.frame(ids.intent(&m, op & 0x40 != 0));
                let hs = heights;
                let ground = move |x: f32, y: f32| -> Option<f32> {
                    if !(x.is_finite() && y.is_finite()) {
                        return None;
                    }
                    let i = (x.floor() as i64).rem_euclid(4) as usize;
                    let j = (y.floor() as i64).rem_euclid(4) as usize;
                    let h = hs[j * 4 + i];
                    (h.to_bits() != 0x7fc0_0001).then_some(h)
                };
                for _ in 0..step.advance(frame) {
                    let intent = latch.tick();
                    fly.tick(&intent, step.dt() as f32);
                    let _ = walker.tick(&intent, &ground, step.dt() as f32);
                    assert!(fly.position().is_finite() && fly.velocity().is_finite());
                    assert!(walker.position().is_finite() && walker.velocity().is_finite());
                }
                None
            }
        };
        if let Some(ev) = ev {
            m.handle(&ev, now);
        }
    }
    m.update(now);
});
