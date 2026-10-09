// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-input` runtime (docs/specs/keymap.md §7, §9): an arbitrary stream of raw events,
//! context changes, UI-band pushes and pops and frame updates through `InputMapper` with the
//! built-in registry and keymap plus fuzzed contexts and bindings (fuzzed `on` values,
//! several bindings of one button in one UI-band context), feeding the fly and walk
//! controllers over a hostile height field. Events go through `handle` and through
//! `handle_with` with a scripted sink that returns fuzzed verdicts and appends fuzzed band
//! requests; `offer_pending`, `release_all`, focus loss and run-time changes of *T*, *W*, *H*
//! are interleaved (§7.9). Nothing may panic, hang or grow without bound, and controller
//! state must stay finite. Checked on every run: (c) and (p) of §7.9 — no End before its
//! Start, at most one End per Start, none for a hold whose Start was never offered, no
//! Repeat outside a Start–End pair, every offered Start ends after `release_all` and
//! `offer_pending`; and, when every event went through `handle_with`, an action whose Starts
//! are always consumed never shows in the snapshot.

#![no_main]

use std::collections::HashMap;
use std::time::Duration;

use libfuzzer_sys::fuzz_target;
use tessera_input::{
    ActionDef, ActionId, Activation, ActivationTiming, AnalogAxis, BandRequests, BindingSpec,
    Button, Chord, ContextDef, ContextId, FixedTimestep, FlyConfig, FlyController, HoldId,
    InputMapper, IntentLatch, KeyCode, LookSettings, Modifiers, MouseButton, OfferPhase,
    OfferedEvent, PadButton, RawEvent, StandardActions, Stroke, Verdict, WalkConfig,
    WalkController, WheelDirection, default_keymap, default_registry,
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

/// Keys the fuzzed bindings use, so presses hit them often.
const KEYS: [KeyCode; 8] = [
    KeyCode::Left,
    KeyCode::Enter,
    KeyCode::Escape,
    KeyCode::Tab,
    KeyCode::W,
    KeyCode::G,
    KeyCode::ShiftLeft,
    KeyCode::ControlLeft,
];

fn button_of(x: u8) -> Button {
    match x % 12 {
        0..=7 => Button::Key(KEYS[usize::from(x % 8)]),
        8 => Button::Mouse(MouseButton::Left),
        9 => Button::Mouse(MouseButton::Right),
        10 => Button::Wheel(WheelDirection::Up),
        _ => Button::Pad(PadButton::South),
    }
}

/// What the scripted sink has seen of each hold.
#[derive(Default)]
struct Tracker {
    /// Hold → whether its End was offered.
    holds: HashMap<HoldId, bool>,
    /// Holds whose Start the sink consumed, of the always-consumed action.
    consumed_action: Option<ActionId>,
}

impl Tracker {
    fn see(&mut self, e: &OfferedEvent) {
        let Some(a) = e.as_action() else {
            return;
        };
        match a.phase {
            OfferPhase::Start => {
                assert!(
                    self.holds.insert(a.hold, false).is_none(),
                    "(c) a second Start for {:?}",
                    a.hold
                );
                assert!(
                    !a.command.is_empty(),
                    "(§2.2) an offered hold has a command"
                );
            }
            OfferPhase::Repeat => {
                assert_eq!(
                    self.holds.get(&a.hold),
                    Some(&false),
                    "Repeat outside its Start and End: {a:?}"
                );
            }
            OfferPhase::End => {
                let ended = self
                    .holds
                    .get_mut(&a.hold)
                    .unwrap_or_else(|| panic!("(p) End without an offered Start: {a:?}"));
                assert!(!*ended, "(c) a second End: {a:?}");
                *ended = true;
            }
        }
    }
}

/// The scripted sink: verdicts and band requests from the script bytes.
fn sink<'a>(
    script: &'a mut Bytes<'_>,
    tracker: &'a mut Tracker,
    contexts: &'a [ContextId],
) -> impl FnMut(&OfferedEvent, &mut BandRequests) -> Verdict + 'a {
    move |e, requests| {
        tracker.see(e);
        let s = script.u8().unwrap_or(0);
        if s & 0x10 != 0 {
            let c = contexts[usize::from(script.u8().unwrap_or(0)) % contexts.len()];
            let owner = u64::from(s >> 6);
            if s & 0x20 != 0 {
                requests.push(c, owner, i32::from(script.u8().unwrap_or(0) as i8));
            } else {
                requests.pop(c, owner);
            }
        }
        let always = e
            .as_action()
            .is_some_and(|a| Some(a.action) == tracker.consumed_action);
        Verdict::from_bool(always || s & 1 != 0)
    }
}

fuzz_target!(|data: &[u8]| {
    let mut b = Bytes(data);
    let Some(header) = b.u8() else { return };
    let ui_only = header & 1 != 0;

    let mut registry = default_registry();
    let ids = StandardActions::new(&registry).expect("built-in ids");
    // Fuzzed contexts and actions, all with command names so they are offered.
    let fz_ctx = [
        registry
            .add_context(ContextDef::new("fz.ui1", 0).ui())
            .expect("ctx"),
        registry
            .add_context(ContextDef::new("fz.ui2", 5).ui().with_opaque_keys())
            .expect("ctx"),
        registry
            .add_context(ContextDef::new("fz.ui3", 0).ui().opaque().with_text())
            .expect("ctx"),
        registry
            .add_context(ContextDef::new("fz.pt", 50).pass_through())
            .expect("ctx"),
        registry
            .add_context(ContextDef::new("fz.game", 25).opaque())
            .expect("ctx"),
    ];
    let fz_actions = [
        registry
            .add_action(
                ActionDef::button("fz.a")
                    .with_command("FzA")
                    .with_repeat(true),
            )
            .expect("action"),
        registry
            .add_action(ActionDef::button("fz.b").with_command("FzB"))
            .expect("action"),
        registry
            .add_action(ActionDef::axis("fz.c").with_command("FzC"))
            .expect("action"),
        registry
            .add_action(ActionDef::button("fz.always").with_command("FzAlways"))
            .expect("action"),
    ];
    let names = ["fz.ui1", "fz.ui2", "fz.ui3", "fz.pt", "fz.game"];
    let anames = ["fz.a", "fz.b", "fz.c", "fz.always"];
    let mut keymap = default_keymap();
    let Some(n) = b.u8() else { return };
    for _ in 0..(n % 24) {
        let (Some(x), Some(y), Some(z)) = (b.u8(), b.u8(), b.u8()) else {
            break;
        };
        let ctx = names[usize::from(x) % names.len()];
        let action = anames[usize::from(x >> 3) % anames.len()];
        let mut buttons = vec![button_of(y)];
        if z & 0x80 != 0 {
            buttons.push(button_of(y >> 4));
        }
        let Ok(stroke) = Stroke::new(Modifiers::from_bits_truncate(z >> 4 & 0x7), buttons) else {
            continue;
        };
        let strokes = if z & 0x08 != 0 {
            vec![stroke.clone(), stroke]
        } else {
            vec![stroke]
        };
        let Ok(chord) = Chord::new(strokes) else {
            continue;
        };
        let on = Activation::ALL[usize::from(z & 0x7) % Activation::ALL.len()];
        let chord = chord.with_activation(on).unwrap_or_else(|_| {
            // Not allowed on this binding (§5.3): keep it as a `press` binding.
            Chord::parse(&chord_text(y)).expect("single button")
        });
        let mut list = keymap
            .get(ctx, action)
            .map(<[_]>::to_vec)
            .unwrap_or_default();
        list.push(BindingSpec::digital(chord, 1.0));
        let _ = keymap.set(ctx, action, list);
    }
    // `fz.always` is consumed at every Start; bind it on the plain left button and
    // Enter in a UI-band context and in `pointer`.
    let mut tracker = Tracker {
        consumed_action: Some(fz_actions[3]),
        ..Tracker::default()
    };
    let always = vec![
        BindingSpec::parse("MouseLeft").expect("binding"),
        BindingSpec::parse("Enter").expect("binding"),
    ];
    let _ = keymap.set("fz.ui1", "fz.always", always.clone());
    let _ = keymap.set("pointer", "fz.always", always);

    let all_contexts: Vec<ContextId> = registry.contexts().map(|(id, _)| id).collect();
    let (mut m, _) = InputMapper::new(registry, &keymap);
    ids.enter_walk_mode(&mut m);
    if header & 2 != 0 {
        m.set_command_ids([("FzA", 1u32), ("FzB", 2), ("FzC", 3), ("FzAlways", 4)]);
        m.set_repeat_commands(Some([1u32, 3]));
    }
    let _ = fz_ctx;

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
    // The sink's script is the tail of the input, read backwards in effect: the
    // second half of the bytes.
    let split = b.0.len() / 2;
    let (events, script) = b.0.split_at(split);
    let mut b = Bytes(events);
    let mut script = Bytes(script);
    let mut ops = 0;
    while let Some(op) = b.u8() {
        ops += 1;
        if ops > 4096 {
            break;
        }
        let ev = match op % 20 {
            0 => {
                let Some(x) = b.u8() else { break };
                let key = if x & 0x80 != 0 {
                    KeyCode::ALL[usize::from(x) % KeyCode::ALL.len()]
                } else {
                    KEYS[usize::from(x) % KEYS.len()]
                };
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
                let c = all_contexts[usize::from(x) % all_contexts.len()];
                m.set_active(c, op & 0x40 != 0);
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
            12 => {
                // A direct band push or pop: any context (game-band ones are refused),
                // any owner (0 included), any priority.
                let (Some(x), Some(o), Some(p)) = (b.u8(), b.u8(), b.u8()) else {
                    break;
                };
                let c = all_contexts[usize::from(x) % all_contexts.len()];
                let owner = if o & 0x80 != 0 {
                    u64::MAX - u64::from(o)
                } else {
                    u64::from(o & 3)
                };
                if op & 0x40 != 0 {
                    m.band_push(c, owner, i32::from(p as i8));
                } else {
                    m.band_pop(c, owner);
                }
                None
            }
            13 => {
                // Up to and past the 256-entry limit.
                let Some(x) = b.u8() else { break };
                let c = all_contexts[usize::from(x) % all_contexts.len()];
                for owner in 0..300u64 {
                    m.band_push(c, owner, i32::from(x as i8));
                }
                assert!(m.band().len() <= tessera_input::MAX_BAND_ENTRIES);
                None
            }
            14 => {
                let mut s = sink(&mut script, &mut tracker, &all_contexts);
                m.offer_pending(&mut s);
                None
            }
            15 => {
                m.release_all();
                None
            }
            16 => {
                // Run-time changes of T, W, H (with or without a scale change).
                let (Some(t), Some(w), Some(h)) = (b.u32(), b.u8(), b.u8()) else {
                    break;
                };
                let timing = ActivationTiming::new(t % 10_000, u32::from(w), u32::from(h));
                assert!((100..=5000).contains(&timing.time_ms()));
                assert!((2..=64).contains(&timing.width_px()));
                if op & 0x40 != 0 {
                    let cursor = m.cursor().map(|(x, y)| (x * 2.0, y * 2.0));
                    m.rescale(timing, cursor);
                } else {
                    m.set_timing(timing);
                }
                None
            }
            17 => {
                let (Some(x), Some(y)) = (b.f32(), b.f32()) else {
                    break;
                };
                Some(RawEvent::CursorMoved {
                    x: f64::from(x),
                    y: f64::from(y),
                })
            }
            18 => Some(RawEvent::CursorLeft),
            _ => {
                // End of frame.
                let Some(x) = b.u32() else { break };
                if !ui_only || op & 0x40 == 0 {
                    // With a UI attached the frame ends with offer_pending.
                    let mut s = sink(&mut script, &mut tracker, &all_contexts);
                    m.offer_pending(&mut s);
                }
                let ms = u64::from(x % 2_000_000);
                now = now.saturating_add(Duration::from_millis(ms));
                m.update(now);
                let _ = m.text();
                if ui_only {
                    let st = m.state(fz_actions[3]);
                    assert!(
                        !st.pressed && st.presses == 0,
                        "(c) a consumed Start shows in the snapshot: {st:?}"
                    );
                }
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
            if ui_only || op & 0x20 != 0 {
                let mut s = sink(&mut script, &mut tracker, &all_contexts);
                m.handle_with(&ev, now, &mut s);
            } else {
                m.handle(&ev, now);
            }
        }
        assert!(m.hold_count() <= tessera_input::MAX_HOLDS);
        assert!(m.band().len() <= tessera_input::MAX_BAND_ENTRIES);
    }
    // Every offered Start gets exactly one End (§7.8.4).
    m.release_all();
    let mut s = sink(&mut script, &mut tracker, &all_contexts);
    m.offer_pending(&mut s);
    drop(s);
    assert_eq!(m.queued_ends(), 0);
    for (hold, ended) in &tracker.holds {
        assert!(*ended, "(c) offered Start of {hold:?} never got its End");
    }
    m.update(now);
});

/// The single-button form of a fuzzed stroke, as binding text.
fn chord_text(y: u8) -> String {
    let c = Chord::single(Stroke::button(button_of(y)));
    c.to_string()
}
