// SPDX-License-Identifier: GPL-3.0-or-later
//! The UI input bridge (`keymap.md` §3, §7.2–§7.10, decision 0014):
//! invariants (a)–(w) of §7.9 that `tessera-input` owns, and the worked
//! example of §7.10. Synthetic only; "a UI attached" means `handle_with`
//! with a scripted sink.

use std::collections::HashSet;
use std::time::Duration;

use tessera_input::{
    ActionDef, ActionId, ActivationTiming, BandRequests, BindingSpec, Button, Chord, ContextDef,
    ContextId, InputMapper, KeyCode, Keymap, KeymapWarning, ModifierPolicy, MouseButton,
    OfferPhase, OfferedEvent, RawEvent, Registry, StandardActions, UiActions, UiSink, Verdict,
    default_keymap, default_registry,
};

// ----- helpers ---------------------------------------------------------------

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

fn key(k: KeyCode, pressed: bool) -> RawEvent {
    RawEvent::Key {
        key: k,
        pressed,
        repeat: false,
    }
}

fn repeat(k: KeyCode) -> RawEvent {
    RawEvent::Key {
        key: k,
        pressed: true,
        repeat: true,
    }
}

fn mouse(b: MouseButton, pressed: bool) -> RawEvent {
    RawEvent::MouseButton { button: b, pressed }
}

fn at(x: f64, y: f64) -> RawEvent {
    RawEvent::CursorMoved { x, y }
}

const L: MouseButton = MouseButton::Left;
const R: MouseButton = MouseButton::Right;

type Hook = Box<dyn FnMut(&OfferedEvent, &mut BandRequests)>;

/// A scripted sink: records every offer, consumes the Starts of the actions
/// in `consume` (or every Start with `consume_all`), and runs `hook` to add
/// band requests.
#[derive(Default)]
struct Rec {
    log: Vec<OfferedEvent>,
    consume: HashSet<ActionId>,
    consume_all: bool,
    hook: Option<Hook>,
}

impl UiSink for Rec {
    fn offer(&mut self, event: &OfferedEvent, requests: &mut BandRequests) -> Verdict {
        self.log.push(event.clone());
        if let Some(h) = self.hook.as_mut() {
            h(event, requests);
        }
        match event {
            OfferedEvent::Action(a) if a.phase == OfferPhase::Start => {
                Verdict::from_bool(self.consume_all || self.consume.contains(&a.action))
            }
            _ => Verdict::NotConsumed,
        }
    }
}

impl Rec {
    fn consuming(actions: &[ActionId]) -> Self {
        Self {
            consume: actions.iter().copied().collect(),
            ..Self::default()
        }
    }

    /// The log as text, then cleared.
    fn take(&mut self, m: &InputMapper) -> Vec<String> {
        std::mem::take(&mut self.log)
            .iter()
            .map(|e| describe(m, e))
            .collect()
    }
}

fn describe(m: &InputMapper, e: &OfferedEvent) -> String {
    match e {
        OfferedEvent::Action(a) => {
            let name = &m.registry().action_def(a.action).unwrap().name;
            let phase = match a.phase {
                OfferPhase::Start => "Start",
                OfferPhase::Repeat => "Repeat",
                OfferPhase::End => "End",
            };
            format!("{phase} {name}")
        }
        OfferedEvent::Text { ch, .. } => format!("Text {ch}"),
        OfferedEvent::PointerMoved { x, y, .. } => format!("Move {x},{y}"),
        OfferedEvent::PointerLeft { .. } => "Left".into(),
        OfferedEvent::FocusLost { .. } => "FocusLost".into(),
    }
}

/// The built-ins in walk mode with `pointer` active, as the client sets
/// them up when it attaches a UI.
fn defaults() -> (InputMapper, StandardActions, UiActions) {
    let reg = default_registry();
    let ids = StandardActions::new(&reg).unwrap();
    let ui = UiActions::new(&reg).unwrap();
    let (mut m, w) = InputMapper::new(reg, &default_keymap());
    assert!(w.is_empty(), "{w:?}");
    ids.enter_walk_mode(&mut m);
    m.activate(ui.pointer);
    (m, ids, ui)
}

/// Default registry plus extra actions, and a keymap laid over the
/// defaults.
fn with_extra(
    actions: &[ActionDef],
    contexts: &[ContextDef],
    doc: &str,
) -> (InputMapper, StandardActions, UiActions) {
    let mut reg = default_registry();
    for c in contexts {
        reg.add_context(c.clone()).unwrap();
    }
    for a in actions {
        reg.add_action(a.clone()).unwrap();
    }
    let ids = StandardActions::new(&reg).unwrap();
    let ui = UiActions::new(&reg).unwrap();
    let km = default_keymap().overlay(&Keymap::parse(doc).unwrap());
    let (mut m, w) = InputMapper::new(reg, &km);
    assert!(w.is_empty(), "{w:?}");
    ids.enter_walk_mode(&mut m);
    m.activate(ui.pointer);
    (m, ids, ui)
}

fn id(m: &InputMapper, name: &str) -> ActionId {
    m.registry().action(name).unwrap()
}

fn ctx(m: &InputMapper, name: &str) -> ContextId {
    m.registry().context(name).unwrap()
}

/// `(presses, releases, pressed)` after an update.
fn edges(m: &InputMapper, a: ActionId) -> (u16, u16, bool) {
    let s = m.state(a);
    (s.presses, s.releases, s.pressed)
}

// ----- (a) activation timing --------------------------------------------------

#[test]
fn a_click_timing_sequence() {
    let mut reg = Registry::new();
    let c = reg.add_context(ContextDef::new("c", 0)).unwrap();
    for n in ["p", "r", "t", "d", "dn"] {
        reg.add_action(ActionDef::button(n)).unwrap();
    }
    let doc = r#"
[bindings.c]
p = ["MouseLeft"]
r = [{ input = "MouseLeft", on = "release" }]
t = [{ input = "MouseLeft", on = "tap" }]
d = [{ input = "MouseLeft", on = "double" }]
dn = [{ input = "MouseLeft", on = "double_nearby" }]
"#;
    for (second, nearby) in [((101.0, 99.0), true), ((101.0, 102.0), false)] {
        let (mut m, w) = InputMapper::new(reg.clone(), &Keymap::parse(doc).unwrap());
        assert!(w.is_empty(), "{w:?}");
        m.activate(c);
        m.set_timing(ActivationTiming::new(500, 4, 4));
        let [p, r, t, d, dn] = ["p", "r", "t", "d", "dn"].map(|n| id(&m, n));
        m.handle(&at(100.0, 100.0), ms(0));
        m.handle(&mouse(L, true), ms(0));
        m.update(ms(0));
        assert_eq!(edges(&m, p), (1, 0, true));
        assert_eq!(edges(&m, d), (0, 0, false));
        m.handle(&mouse(L, false), ms(200));
        m.update(ms(200));
        assert_eq!(edges(&m, p), (0, 1, false));
        assert_eq!(edges(&m, r), (1, 1, false), "Up");
        assert_eq!(edges(&m, t), (1, 1, false), "Tap: 200 <= 250");
        m.handle(&at(second.0, second.1), ms(400));
        m.handle(&mouse(L, true), ms(400));
        m.update(ms(400));
        assert_eq!(edges(&m, p), (1, 0, true));
        assert_eq!(edges(&m, d), (1, 0, true), "DblClickDown: 400 <= 500");
        assert_eq!(edges(&m, dn).2, nearby, "NearbyDown at {second:?}");
        m.handle(&mouse(L, false), ms(700));
        m.update(ms(700));
        assert_eq!(edges(&m, r), (1, 1, false));
        assert_eq!(edges(&m, t), (0, 0, false), "no Tap: 300 > 250");
        assert_eq!(edges(&m, d), (0, 1, false));
    }
}

// ----- (b) band ordering -----------------------------------------------------

#[test]
fn b_push_and_pop_order() {
    let mut reg = Registry::new();
    let mp = reg.add_context(ContextDef::new("mp", 0).ui()).unwrap();
    let mf = reg.add_context(ContextDef::new("mf", 0).ui()).unwrap();
    let (mut m, _) = InputMapper::new(reg, &Keymap::new());
    let (p, f) = (1u64, 2u64);
    m.band_push(mp, p, 0);
    m.band_push(mp, p, 49);
    m.band_push(mf, f, 50);
    let stack: Vec<_> = m
        .band()
        .iter()
        .map(|e| (e.context, e.owner, e.priority))
        .collect();
    assert_eq!(stack, [(mf, f, 50), (mp, p, 49), (mp, p, 0)]);
    assert_eq!(
        m.ordered(),
        [mf, mp],
        "each context once, at its first entry"
    );
    // An identical push changes nothing and does not move the entry.
    m.band_push(mp, p, 49);
    assert_eq!(m.band().len(), 3);
    m.band_pop(mf, f);
    m.band_pop(mp, p);
    assert!(
        m.band().is_empty(),
        "a pop removes the pair at every priority"
    );
    m.band_push(mp, p, 0);
    let stack: Vec<_> = m
        .band()
        .iter()
        .map(|e| (e.context, e.owner, e.priority))
        .collect();
    assert_eq!(stack, [(mp, p, 0)]);
    // Equal priorities: the newest first.
    m.band_push(mf, f, 0);
    assert_eq!(m.band()[0].context, mf);
}

#[test]
fn band_limits_and_game_band_refusal() {
    let (mut m, ids, ui) = defaults();
    m.band_push(ids.movement, 3, 0);
    assert!(
        m.band().is_empty(),
        "a push naming a game-band context is refused"
    );
    for owner in 0..300u64 {
        m.band_push(ui.ui_scroll_mouse, owner + 1, 0);
    }
    assert_eq!(m.band().len(), tessera_input::MAX_BAND_ENTRIES);
}

// ----- (c) consumed Starts and their Ends ------------------------------------

#[test]
fn c_consumed_start_never_shows_and_gets_one_end() {
    for how in ["deactivate", "set_keymap", "release_all", "release"] {
        let (mut m, _, ui) = defaults();
        let mut rec = Rec::consuming(&[ui.select_left]);
        m.handle_with(&at(10.0, 10.0), ms(0), &mut rec);
        m.handle_with(&mouse(L, true), ms(0), &mut rec);
        m.update(ms(0));
        assert_eq!(edges(&m, ui.select_left), (0, 0, false), "{how}");
        match how {
            "deactivate" => m.deactivate(ui.pointer),
            "set_keymap" => {
                let km = m.keymap().clone();
                let _ = m.set_keymap(&km);
            }
            "release_all" => m.release_all(),
            _ => m.handle_with(&mouse(L, false), ms(100), &mut rec),
        }
        // Ends caused outside an offer wait in the end queue.
        m.offer_pending(&mut rec);
        m.offer_pending(&mut rec);
        m.update(ms(100));
        let log = rec.take(&m);
        assert_eq!(
            log.iter().filter(|l| *l == "End select_left").count(),
            1,
            "{how}: {log:?}"
        );
        assert_eq!(edges(&m, ui.select_left), (0, 0, false), "{how}");
    }
}

// ----- (d) and the worked example: double click over a plugin button --------

#[test]
fn d_double_click_order_over_a_button() {
    let (mut m, ids, ui) = defaults();
    m.set_timing(ActivationTiming::new(500, 4, 4));
    let mut rec = Rec::consuming(&[ui.select_left, ui.select_double_left, ui.tap_left]);
    m.handle_with(&at(50.0, 50.0), ms(0), &mut rec);
    m.handle_with(&mouse(L, true), ms(0), &mut rec);
    assert!(m.is_marked(Button::Mouse(L)), "UI-owned press marks left");
    m.handle_with(&mouse(L, false), ms(150), &mut rec);
    m.handle_with(&mouse(L, true), ms(300), &mut rec);
    m.handle_with(&mouse(L, false), ms(600), &mut rec);
    m.offer_pending(&mut rec);
    m.update(ms(600));
    assert_eq!(
        rec.take(&m),
        [
            "Move 50,50",
            "Start select_left",
            "End select_left",
            "Start tap_left",
            "End tap_left",
            "Start select_left",
            "Start select_double_left",
            "End select_left",
            "End select_double_left",
        ]
    );
    for a in [
        ui.select_left,
        ui.select_double_left,
        ui.tap_left,
        ids.move_forward,
    ] {
        assert_eq!(edges(&m, a), (0, 0, false), "the game sees none of it");
    }
}

#[test]
fn worked_example_clicks_over_the_world() {
    let (mut m, _, ui) = defaults();
    let mut rec = Rec::default();
    let mut presses = [0u16; 3];
    let mut frame = |m: &mut InputMapper, rec: &mut Rec, e: RawEvent, t: u64| {
        m.handle_with(&e, ms(t), rec);
        m.offer_pending(rec);
        m.update(ms(t));
        for (i, a) in [ui.select_left, ui.tap_left, ui.select_double_left]
            .into_iter()
            .enumerate()
        {
            presses[i] += m.state(a).presses;
        }
    };
    frame(&mut m, &mut rec, at(5.0, 5.0), 0);
    frame(&mut m, &mut rec, mouse(L, true), 0);
    assert!(!m.is_marked(Button::Mouse(L)), "declined: not marked");
    frame(&mut m, &mut rec, mouse(L, false), 150);
    frame(&mut m, &mut rec, mouse(L, true), 300);
    frame(&mut m, &mut rec, mouse(L, false), 600);
    assert_eq!(presses, [2, 1, 1]);
}

#[test]
fn worked_example_right_clicks() {
    let (mut m, ids, ui) = defaults();
    // Over the world: right at 900, released at 1000 (tap), again at 1200.
    let mut rec = Rec::default();
    m.handle_with(&at(20.0, 20.0), ms(800), &mut rec);
    m.handle_with(&mouse(R, true), ms(900), &mut rec);
    m.handle_with(&mouse(R, false), ms(1000), &mut rec);
    m.handle_with(&mouse(R, true), ms(1200), &mut rec);
    m.update(ms(1200));
    let log = rec.take(&m);
    assert_eq!(
        log,
        [
            "Move 20,20",
            "Start mouselook",
            "End mouselook",
            "Start select_right",
            "End select_right",
            "Start select_double_right",
            "Start mouselook",
        ]
    );
    assert!(m.pressed(ids.mouselook), "mouse-look starts again");

    // On a plugin button B: the double-select is consumed, mouse-look does
    // not start.
    let (mut m, ids, ui2) = defaults();
    let mut rec = Rec::consuming(&[ui2.select_right, ui2.select_double_right]);
    m.handle_with(&at(20.0, 20.0), ms(0), &mut rec);
    m.handle_with(&mouse(R, true), ms(0), &mut rec);
    m.handle_with(&mouse(R, false), ms(100), &mut rec);
    m.handle_with(&mouse(R, true), ms(300), &mut rec);
    m.update(ms(300));
    assert_eq!(
        rec.take(&m),
        [
            "Move 20,20",
            "Start mouselook",
            "End mouselook",
            "Start select_right",
            "End select_right",
            "Start select_double_right",
        ]
    );
    assert!(!m.pressed(ids.mouselook));
    let _ = ui;
}

#[test]
fn worked_example_wheel_onto_a_list() {
    let (mut m, ids, ui) = defaults();
    let sm = ui.ui_scroll_mouse;
    let mut rec = Rec {
        hook: Some(Box::new(move |e, r| {
            if let OfferedEvent::PointerMoved { x, .. } = e
                && *x > 100.0
            {
                r.push(sm, 77, 0);
            }
        })),
        ..Rec::default()
    };
    m.handle_with(&at(200.0, 10.0), ms(0), &mut rec);
    m.handle_with(&RawEvent::Wheel { dx: 0.0, dy: 1.0 }, ms(0), &mut rec);
    m.offer_pending(&mut rec);
    m.update(ms(0));
    assert_eq!(
        rec.take(&m),
        ["Move 200,10", "Start scroll_up", "End scroll_up"]
    );
    assert_eq!(m.delta(ids.zoom), 0.0, "the camera does not zoom");
}

#[test]
fn worked_example_escape_in_a_field() {
    let (mut m, ids, ui) = defaults();
    m.band_push(ui.text_entry, 9, 50);
    let mut rec = Rec::default();
    m.handle_with(&key(KeyCode::Escape, true), ms(0), &mut rec);
    m.update(ms(0));
    let log = rec.take(&m);
    assert_eq!(log, ["Start menu"]);
    let e = m.registry().action_def(ids.menu).unwrap();
    assert_eq!(e.name, "menu");
    assert!(
        m.just_pressed(ids.menu),
        "(f) a declined Escape presses menu"
    );
}

// ----- (e) no UI-band entry: the pre-0014 snapshot ---------------------------

/// The actions that existed before decision 0014.
fn old_actions(ids: &StandardActions) -> Vec<ActionId> {
    vec![
        ids.menu,
        ids.screenshot,
        ids.toggle_fly,
        ids.toggle_debug,
        ids.open_chat,
        ids.mouselook,
        ids.look_x,
        ids.look_y,
        ids.zoom,
        ids.reset_camera,
        ids.move_forward,
        ids.strafe,
        ids.turn,
        ids.jump,
        ids.walk,
        ids.autorun,
        ids.text_backspace,
    ]
}

fn run_both(events: &[(RawEvent, u64)]) {
    let (mut on, ids, _) = defaults();
    let (mut off, _, ui) = defaults();
    off.deactivate(ui.pointer);
    for &(e, t) in events {
        on.handle(&e, ms(t));
        off.handle(&e, ms(t));
        on.update(ms(t));
        off.update(ms(t));
        for a in old_actions(&ids) {
            assert_eq!(on.state(a), off.state(a), "{e:?} at {t}");
        }
    }
}

#[test]
fn e_pointer_changes_no_old_action() {
    // A second right press within T at the same position, and a second
    // left press within T while right is held.
    run_both(&[
        (at(10.0, 10.0), 0),
        (mouse(R, true), 0),
        (mouse(R, false), 100),
        (mouse(R, true), 200),
        (mouse(L, true), 250),
        (mouse(L, false), 300),
        (mouse(L, true), 350),
        (mouse(L, false), 900),
        (mouse(R, false), 1000),
    ]);
    let (mut m, ids, _) = defaults();
    for (e, t) in [
        (mouse(R, true), 0),
        (mouse(R, false), 100),
        (mouse(R, true), 200),
    ] {
        m.handle(&e, ms(t));
    }
    m.update(ms(200));
    assert!(m.pressed(ids.mouselook), "mouselook starts again");
    for (e, t) in [
        (mouse(L, true), 250),
        (mouse(L, false), 300),
        (mouse(L, true), 350),
    ] {
        m.handle(&e, ms(t));
    }
    m.update(ms(350));
    assert_eq!(m.value(ids.move_forward), 1.0, "move_forward +1 again");
}

mod props {
    use proptest::prelude::*;

    use super::*;

    fn event() -> impl Strategy<Value = RawEvent> {
        prop_oneof![
            (0..3u8, any::<bool>())
                .prop_map(|(b, p)| mouse([L, R, MouseButton::Middle][usize::from(b)], p)),
            (0..6u8, any::<bool>()).prop_map(|(k, p)| key(
                [
                    KeyCode::W,
                    KeyCode::ShiftLeft,
                    KeyCode::Escape,
                    KeyCode::Enter,
                    KeyCode::Space,
                    KeyCode::ControlLeft
                ][usize::from(k)],
                p
            )),
            (0..3u8, 0..3u8).prop_map(|(x, y)| at(f64::from(x) * 3.0, f64::from(y) * 3.0)),
            (-2i8..=2).prop_map(|n| RawEvent::Wheel {
                dx: 0.0,
                dy: f32::from(n)
            }),
            Just(RawEvent::FocusLost),
            Just(RawEvent::CursorLeft),
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        /// (e): with no UI-band entry, `pointer` active or not gives the same
        /// snapshot for every older action, for any press timing.
        #[test]
        fn e_any_timing(events in proptest::collection::vec((event(), 0u64..400), 1..40)) {
            let mut t = 0;
            let timed: Vec<(RawEvent, u64)> = events
                .into_iter()
                .map(|(e, dt)| {
                    t += dt;
                    (e, t)
                })
                .collect();
            run_both(&timed);
        }

        /// (c), (p): every offered Start gets exactly one End, never before
        /// it, whatever the verdicts and band requests.
        #[test]
        fn c_p_one_end_per_start(
            events in proptest::collection::vec((event(), 0u64..400, any::<u8>()), 1..40),
        ) {
            let (mut m, _, ui) = defaults();
            let ctxs = [ui.text_entry, ui.ui_dialog, ui.ui_clipboard, ui.ui_scroll_mouse,
                ui.ui_eat_keyboard, ui.ui_chat_log, ui.pointer];
            let mut t = 0;
            let mut log: Vec<OfferedEvent> = Vec::new();
            for (e, dt, r) in events {
                t += dt;
                let mut sink = |ev: &OfferedEvent, req: &mut BandRequests| {
                    log.push(ev.clone());
                    let c = ctxs[usize::from(r) % ctxs.len()];
                    match r % 5 {
                        0 => req.push(c, u64::from(r % 3), i32::from(r % 4)),
                        1 => req.pop(c, u64::from(r % 3)),
                        _ => {}
                    }
                    Verdict::from_bool(r & 0x80 != 0)
                };
                if r & 1 == 0 {
                    m.handle_with(&e, ms(t), &mut sink);
                } else {
                    m.handle(&e, ms(t));
                }
                if r & 6 == 0 {
                    m.offer_pending(&mut sink);
                }
                m.update(ms(t));
            }
            m.release_all();
            m.offer_pending(&mut |ev: &OfferedEvent, _: &mut BandRequests| {
                log.push(ev.clone());
                Verdict::NotConsumed
            });
            let mut open = HashSet::new();
            for ev in &log {
                if let OfferedEvent::Action(a) = ev {
                    match a.phase {
                        OfferPhase::Start => prop_assert!(open.insert(a.hold)),
                        OfferPhase::Repeat => prop_assert!(open.contains(&a.hold)),
                        OfferPhase::End => prop_assert!(open.remove(&a.hold), "End without Start"),
                    }
                }
            }
            prop_assert!(open.is_empty(), "Starts without End: {open:?}");
        }
    }
}

// ----- (e2) text_entry under `handle` -----------------------------------------

#[test]
fn e2_text_entry_activated_by_code() {
    let (mut m, ids, ui) = defaults();
    m.handle(&mouse(R, true), ms(0));
    m.handle(&key(KeyCode::W, true), ms(0));
    m.update(ms(0));
    assert!(m.pressed(ids.mouselook) && m.value(ids.move_forward) == 1.0);
    m.activate(ui.text_entry);
    assert!(m.is_active(ui.text_entry));
    let entries: Vec<_> = m
        .band()
        .iter()
        .map(|e| (e.context, e.owner, e.priority))
        .collect();
    assert_eq!(entries, [(ui.text_entry, 0, 100)]);
    m.activate(ui.text_entry);
    assert_eq!(m.band().len(), 1, "activating again changes nothing");
    m.update(ms(1));
    assert!(m.pressed(ids.mouselook), "a held MouseRight survives");
    assert_eq!(m.value(ids.move_forward), 0.0, "a held W is released");
    m.handle(&key(KeyCode::W, false), ms(2));
    m.handle(&key(KeyCode::W, true), ms(3));
    m.handle(&RawEvent::Text('w'), ms(3));
    m.update(ms(3));
    assert_eq!(m.text(), "w");
    assert_eq!(m.value(ids.move_forward), 0.0, "W types instead of moving");
    m.handle(&key(KeyCode::Enter, true), ms(4));
    m.handle(&key(KeyCode::Escape, true), ms(4));
    m.handle(&RawEvent::Wheel { dx: 0.0, dy: 1.0 }, ms(4));
    m.update(ms(4));
    assert!(m.just_pressed(ids.open_chat) && m.just_pressed(ids.menu));
    assert!(m.delta(ids.zoom) != 0.0, "the wheel changes zoom");
    m.band_push(ids.movement, 0, 0);
    assert_eq!(
        m.band().len(),
        1,
        "a band push naming movement changes nothing"
    );
    m.deactivate(ui.text_entry);
    assert!(m.band().is_empty());
}

// ----- (e3) compile-time disabling -------------------------------------------

#[test]
fn e3_disabled_entries() {
    let reg = default_registry();
    let doc = r#"
[bindings.pointer]
select_left = ["G G"]
[bindings.movement]
move_forward = [{ input = "W", on = "tap" }]
"#;
    let (_, w) = InputMapper::new(reg, &default_keymap().overlay(&Keymap::parse(doc).unwrap()));
    assert_eq!(w.len(), 2, "{w:?}");
    assert!(
        w.iter()
            .all(|w| matches!(w, KeymapWarning::Disabled { .. }))
    );
}

// ----- (g), (g2), (h) ---------------------------------------------------------

#[test]
fn g_run_chord_and_quick_presses_with_a_ui() {
    for order in [[L, R], [R, L]] {
        let (mut m, ids, _) = defaults();
        let mut rec = Rec::default();
        m.handle_with(&at(1.0, 1.0), ms(0), &mut rec);
        for (i, b) in order.into_iter().enumerate() {
            m.handle_with(&mouse(b, true), ms(10 * i as u64), &mut rec);
        }
        m.update(ms(20));
        assert_eq!(m.value(ids.move_forward), 1.0, "{order:?}");
        assert!(m.pressed(ids.mouselook));
        m.handle_with(&mouse(L, false), ms(300), &mut rec);
        m.update(ms(300));
        assert!(
            m.pressed(ids.mouselook),
            "mouselook survives releasing left"
        );
        assert_eq!(m.value(ids.move_forward), 0.0);
        // A second left press within T while right is held.
        m.handle_with(&mouse(L, true), ms(400), &mut rec);
        m.update(ms(400));
        assert_eq!(m.value(ids.move_forward), 1.0, "runs again");
        let log = rec.take(&m);
        let tail: Vec<_> = log.iter().rev().take(3).rev().cloned().collect();
        assert_eq!(
            tail,
            [
                "Start select_left",
                "Start select_double_left",
                "Start move_forward"
            ],
            "{order:?}"
        );
    }
}

#[test]
fn g2_hit_element_takes_the_double_right() {
    let (mut m, ids, ui) = defaults();
    let mut rec = Rec::consuming(&[ui.select_double_right]);
    m.handle_with(&at(1.0, 1.0), ms(0), &mut rec);
    m.handle_with(&mouse(R, true), ms(0), &mut rec);
    m.handle_with(&mouse(R, false), ms(50), &mut rec);
    m.handle_with(&mouse(R, true), ms(100), &mut rec);
    m.update(ms(100));
    assert!(!m.pressed(ids.mouselook));
    assert_eq!(rec.take(&m).last().unwrap(), "Start select_double_right");
}

#[test]
fn h_shift_click_selects() {
    let (mut m, _, ui) = defaults();
    let mut rec = Rec::default();
    m.handle_with(&key(KeyCode::ShiftLeft, true), ms(0), &mut rec);
    m.handle_with(&mouse(L, true), ms(0), &mut rec);
    m.update(ms(0));
    assert!(m.pressed(ui.select_left));
    // (m) offered exactly once, in pass 2.
    assert_eq!(
        rec.take(&m)
            .iter()
            .filter(|l| *l == "Start select_left")
            .count(),
        1
    );
    let (mut m, _, _) = defaults();
    m.handle_with(&mouse(L, true), ms(0), &mut rec);
    assert_eq!(rec.take(&m), ["Start select_left"], "(m) once in pass 1");
}

// ----- (i) focused field ------------------------------------------------------

#[test]
fn i_field_clipboard_and_dialog() {
    for clipboard_first in [false, true] {
        let (mut m, ids, ui) = defaults();
        m.band_push(ui.ui_dialog, 1, 0);
        if clipboard_first {
            m.band_push(ui.ui_clipboard, 2, 50);
            m.band_push(ui.text_entry, 2, 50);
        } else {
            m.band_push(ui.text_entry, 2, 50);
            m.band_push(ui.ui_clipboard, 2, 50);
        }
        let mut rec = Rec::default();
        m.handle_with(&key(KeyCode::W, true), ms(0), &mut rec);
        m.handle_with(&RawEvent::Text('w'), ms(0), &mut rec);
        m.handle_with(&key(KeyCode::W, false), ms(0), &mut rec);
        m.handle_with(&key(KeyCode::ControlLeft, true), ms(1), &mut rec);
        m.handle_with(&key(KeyCode::C, true), ms(1), &mut rec);
        m.handle_with(&key(KeyCode::C, false), ms(1), &mut rec);
        m.handle_with(&key(KeyCode::ControlLeft, false), ms(1), &mut rec);
        m.handle_with(&key(KeyCode::Tab, true), ms(2), &mut rec);
        m.update(ms(2));
        assert_eq!(m.value(ids.move_forward), 0.0);
        assert_eq!(m.text(), "", "characters went to the UI, not the snapshot");
        assert_eq!(
            rec.take(&m),
            [
                "Text w",
                "Start text_copy",
                "End text_copy",
                "Start focus_next"
            ],
            "clipboard first: {clipboard_first}"
        );
    }
}

// ----- (j) move coalescing ----------------------------------------------------

#[test]
fn j_moves_are_coalesced() {
    let (mut m, _, _) = defaults();
    let mut rec = Rec::default();
    for i in 0..50 {
        m.handle_with(&at(f64::from(i), 2.0), ms(u64::from(i as u32)), &mut rec);
    }
    m.handle_with(
        &RawEvent::MouseMotion { dx: 1.0, dy: 1.0 },
        ms(60),
        &mut rec,
    );
    m.handle_with(&mouse(L, true), ms(60), &mut rec);
    assert_eq!(rec.take(&m), ["Move 49,2", "Start select_left"]);
    m.offer_pending(&mut rec);
    assert!(rec.take(&m).is_empty());
}

// ----- (k), (l) the wheel over the UI ----------------------------------------

#[test]
fn k_wheel_withheld_while_scroll_mouse_is_pushed() {
    let wheel = RawEvent::Wheel { dx: 0.0, dy: 1.0 };
    // With a UI and the entry pushed.
    let (mut m, ids, ui) = defaults();
    m.band_push(ui.ui_scroll_mouse, 5, 0);
    let mut rec = Rec::default();
    m.handle_with(&wheel, ms(0), &mut rec);
    m.update(ms(0));
    assert_eq!(rec.take(&m), ["Start scroll_up", "End scroll_up"]);
    assert_eq!(m.delta(ids.zoom), 0.0);
    // Without it, zoom changes.
    m.band_pop(ui.ui_scroll_mouse, 5);
    m.handle_with(&wheel, ms(1), &mut rec);
    m.update(ms(1));
    assert!(m.delta(ids.zoom) > 0.0);
    // Under `handle`, activated by code.
    m.activate(ui.ui_scroll_mouse);
    m.handle(&wheel, ms(2));
    m.update(ms(2));
    assert_eq!(m.state(ui.scroll_up).presses, 1);
    assert_eq!(m.delta(ids.zoom), 0.0);
    // Resolved while pushed, popped before update: still withheld.
    m.handle(&wheel, ms(3));
    m.deactivate(ui.ui_scroll_mouse);
    m.update(ms(3));
    assert_eq!(m.delta(ids.zoom), 0.0);
    // The reverse order changes zoom.
    m.handle(&wheel, ms(4));
    m.activate(ui.ui_scroll_mouse);
    m.update(ms(4));
    assert!(m.delta(ids.zoom) > 0.0);
}

#[test]
fn l_move_is_flushed_before_wheel_and_keys() {
    for second in [RawEvent::Wheel { dx: 0.0, dy: 1.0 }, key(KeyCode::F7, true)] {
        let (mut m, ids, ui) = defaults();
        let sm = ui.ui_scroll_mouse;
        let mut rec = Rec {
            hook: Some(Box::new(move |e, r| {
                if matches!(e, OfferedEvent::PointerMoved { .. }) {
                    r.push(sm, 3, 0);
                }
            })),
            ..Rec::default()
        };
        m.handle_with(&at(7.0, 7.0), ms(0), &mut rec);
        m.handle_with(&second, ms(0), &mut rec);
        m.update(ms(0));
        let log = rec.take(&m);
        assert_eq!(log[0], "Move 7,7");
        if matches!(second, RawEvent::Wheel { .. }) {
            assert_eq!(log[1..], ["Start scroll_up", "End scroll_up"]);
            assert_eq!(m.delta(ids.zoom), 0.0);
        } else {
            assert_eq!(log.len(), 1);
        }
    }
}

// ----- (m2) exact modifier-click in pointer -----------------------------------

#[test]
fn m2_exact_binding_in_pointer_wins_pass_one() {
    let (mut m, _, ui) = with_extra(
        &[
            ActionDef::button("test.shift_click").with_command("TestShiftClick"),
            ActionDef::button("test.low").with_modifiers(ModifierPolicy::Subset),
        ],
        &[],
        r#"
[bindings.pointer]
"test.shift_click" = ["Shift+MouseLeft"]
[bindings.movement]
"test.low" = ["MouseLeft"]
"#,
    );
    let mut rec = Rec::default();
    m.handle_with(&key(KeyCode::ShiftLeft, true), ms(0), &mut rec);
    m.handle_with(&mouse(L, true), ms(0), &mut rec);
    m.update(ms(0));
    assert_eq!(rec.take(&m), ["Start test.shift_click"]);
    assert!(!m.pressed(ui.select_left));
    assert!(
        m.pressed(id(&m, "test.low")),
        "pass 2 still reaches movement"
    );
}

// ----- (n) requests during offers --------------------------------------------

#[test]
fn n_requests_take_effect_from_the_next_raw_event() {
    let extra = [ActionDef::button("test.ui_left").with_command("TestUiLeft")];
    let ctxs = [ContextDef::new("test.ui", 0).ui()];
    let doc = "[bindings.\"test.ui\"]\n\"test.ui_left\" = [\"MouseLeft\"]\n";
    let (mut m, _, _) = with_extra(&extra, &ctxs, doc);
    let tu = ctx(&m, "test.ui");
    let sl = id(&m, "select_left");
    let mut rec = Rec {
        hook: Some(Box::new(move |e, r| {
            if let OfferedEvent::Action(a) = e
                && a.action == sl
                && a.phase == OfferPhase::Start
            {
                r.push(tu, 1, 0);
            }
        })),
        ..Rec::default()
    };
    m.handle_with(&mouse(L, true), ms(0), &mut rec);
    assert_eq!(
        rec.take(&m),
        ["Start select_left"],
        "not consulted this event"
    );
    m.handle_with(&mouse(L, false), ms(1000), &mut rec);
    rec.hook = None;
    m.handle_with(&mouse(L, true), ms(2000), &mut rec);
    assert_eq!(
        rec.take(&m).last().unwrap(),
        "Start test.ui_left",
        "consulted for the next raw event"
    );

    // A push requested during the step-2 move offer counts for that event.
    let (mut m, _, _) = with_extra(&extra, &ctxs, doc);
    let tu = ctx(&m, "test.ui");
    let mut rec = Rec {
        hook: Some(Box::new(move |e, r| {
            if matches!(e, OfferedEvent::PointerMoved { .. }) {
                r.push(tu, 1, 0);
            }
        })),
        ..Rec::default()
    };
    m.handle_with(&at(3.0, 3.0), ms(0), &mut rec);
    m.handle_with(&mouse(L, true), ms(0), &mut rec);
    assert_eq!(rec.take(&m), ["Move 3,3", "Start test.ui_left"]);

    // And during an End offered in step 1.
    let (mut m, _, ui) = with_extra(&extra, &ctxs, doc);
    let tu = ctx(&m, "test.ui");
    let mut rec = Rec::default();
    m.handle_with(&mouse(R, true), ms(0), &mut rec);
    m.deactivate(ui.pointer);
    m.activate(ui.pointer);
    m.release_all(); // the mouselook End waits in the end queue
    rec.hook = Some(Box::new(move |e, r| {
        if let OfferedEvent::Action(a) = e
            && a.phase == OfferPhase::End
        {
            r.push(tu, 1, 0);
        }
    }));
    rec.take(&m);
    m.handle_with(&mouse(L, true), ms(10), &mut rec);
    assert_eq!(rec.take(&m), ["End mouselook", "Start test.ui_left"]);
}

// ----- (o), (o2) the mark -----------------------------------------------------

#[test]
fn o_release_binding_skipped_after_a_ui_owned_press() {
    let extra = [ActionDef::button("test.rel")];
    let doc = "[bindings.movement]\n\"test.rel\" = [{ input = \"MouseLeft\", on = \"release\" }]\n";
    for over_panel in [false, true] {
        let (mut m, _, ui) = with_extra(&extra, &[], doc);
        let rel = id(&m, "test.rel");
        let mut rec = if over_panel {
            Rec::consuming(&[ui.select_left, ui.tap_left])
        } else {
            Rec::default()
        };
        m.handle_with(&mouse(L, true), ms(0), &mut rec);
        m.handle_with(&mouse(L, false), ms(100), &mut rec);
        m.update(ms(100));
        let log = rec.take(&m);
        assert!(log.contains(&"Start tap_left".to_owned()), "{log:?}");
        assert_eq!(
            m.state(rel).presses,
            u16::from(!over_panel),
            "over panel: {over_panel}"
        );
    }
}

#[test]
fn o2_marked_left_does_not_complete_the_run_chord() {
    let (mut m, ids, ui) = defaults();
    let mut rec = Rec::consuming(&[ui.select_left]);
    m.handle_with(&mouse(L, true), ms(0), &mut rec);
    assert!(m.is_marked(Button::Mouse(L)));
    m.handle_with(&mouse(R, true), ms(10), &mut rec);
    m.update(ms(10));
    assert!(m.pressed(ids.mouselook));
    assert_eq!(m.value(ids.move_forward), 0.0, "no run");
    m.handle_with(&mouse(L, true), ms(20), &mut rec);
    m.handle_with(&mouse(L, false), ms(30), &mut rec);
    let log = rec.take(&m);
    assert_eq!(
        log,
        [
            "Start select_left",
            "Start mouselook",
            "End select_left",
            "Start tap_left",
            "End tap_left"
        ]
    );
    assert!(!m.is_held(Button::Mouse(L)) && !m.is_marked(Button::Mouse(L)));
}

// ----- (p) drops ------------------------------------------------------------

#[test]
fn p_second_hold_dropped_when_the_first_offer_pops_its_context() {
    let extra = [
        ActionDef::button("test.a1").with_command("TestA1"),
        ActionDef::button("test.a2").with_command("TestA2"),
    ];
    let ctxs = [ContextDef::new("test.pop", 0).ui()];
    let doc = "[bindings.\"test.pop\"]\n\"test.a1\" = [\"K\"]\n\"test.a2\" = [\"K\"]\n";
    let (mut m, _, _) = with_extra(&extra, &ctxs, doc);
    let tp = ctx(&m, "test.pop");
    let a2 = id(&m, "test.a2");
    m.band_push(tp, 4, 0);
    let mut rec = Rec {
        hook: Some(Box::new(move |e, r| {
            if let OfferedEvent::Action(a) = e
                && a.phase == OfferPhase::Start
            {
                r.pop(tp, 4);
            }
        })),
        ..Rec::default()
    };
    m.handle_with(&key(KeyCode::K, true), ms(0), &mut rec);
    m.handle_with(&key(KeyCode::K, false), ms(5), &mut rec);
    m.update(ms(5));
    assert_eq!(rec.take(&m), ["Start test.a1", "End test.a1"]);
    assert_eq!(edges(&m, a2), (0, 0, false));
    // No End for a hold made under `handle`.
    m.band_push(tp, 4, 0);
    m.handle(&key(KeyCode::K, true), ms(10));
    m.band_pop(tp, 4);
    m.offer_pending(&mut rec);
    assert!(rec.take(&m).is_empty());
}

// ----- (q) modifier policies ---------------------------------------------------

#[test]
fn q_modifier_policies() {
    let (mut m, _, ui) = defaults();
    m.activate(ui.text_entry);
    let check = |m: &mut InputMapper, mods: &[KeyCode], k: KeyCode, want: &[ActionId]| {
        for &mk in mods {
            m.handle(&key(mk, true), ms(0));
        }
        m.handle(&key(k, true), ms(0));
        m.update(ms(0));
        let fired: Vec<ActionId> = m
            .registry()
            .actions()
            .map(|(a, _)| a)
            .filter(|a| m.state(*a).presses > 0)
            .collect();
        assert_eq!(fired, want, "{mods:?} {k:?}");
        m.handle(&key(k, false), ms(0));
        for &mk in mods {
            m.handle(&key(mk, false), ms(0));
        }
        m.update(ms(0));
    };
    use KeyCode::{ControlLeft as Ctrl, Left, ShiftLeft as Shift, Tab, Up};
    check(&mut m, &[Shift], Left, &[ui.text_char_left]);
    check(&mut m, &[Ctrl, Shift], Left, &[ui.text_word_left]);
    check(&mut m, &[Shift], Up, &[ui.text_line_up]);
    check(&mut m, &[Ctrl], Up, &[ui.text_line_up]);
    m.deactivate(ui.text_entry);
    m.activate(ui.ui_dialog);
    // Ctrl+Tab is not `focus_next` (exact policy); it is the global `target_nearest_player`
    // of the P2 defaults (`keymap.md` §8.2). Plain Tab goes to the UI band first.
    let nearest_player = m
        .registry()
        .action("target_nearest_player")
        .expect("P2 action");
    check(&mut m, &[Ctrl], Tab, &[nearest_player]);
    check(&mut m, &[], Tab, &[ui.focus_next]);
}

// ----- (r), (w) timing --------------------------------------------------------

#[test]
fn r_smallest_rectangle() {
    let (mut m, _, ui) = defaults();
    m.set_timing(ActivationTiming::from_logical(500, 1.0, 1.0, 1.0));
    assert_eq!(m.timing().width_px(), 2);
    for (x, want) in [(10.0, 1), (11.0, 0)] {
        m.handle(&at(10.0, 10.0), ms(0));
        m.handle(&mouse(L, true), ms(0));
        m.handle(&mouse(L, false), ms(10));
        m.handle(&at(x, 10.0), ms(20));
        m.handle(&mouse(L, true), ms(20));
        m.handle(&mouse(L, false), ms(30));
        m.update(ms(30));
        assert_eq!(m.state(ui.select_double_left).presses, want, "x = {x}");
        m.handle(&mouse(L, true), ms(5000));
        m.handle(&mouse(L, false), ms(5000));
        m.update(ms(5000));
    }
}

#[test]
fn w_repeats_are_not_recorded_and_scale_change_resets_positions() {
    let extra = [ActionDef::button("test.d"), ActionDef::button("test.dn")];
    let doc = r#"
[bindings.global]
"test.d" = [{ input = "K", on = "double" }, { input = "MouseLeft", on = "double" }]
"test.dn" = [{ input = "MouseLeft", on = "double_nearby" }]
"#;
    let (mut m, _, _) = with_extra(&extra, &[], doc);
    let (d, dn) = (id(&m, "test.d"), id(&m, "test.dn"));
    m.handle(&key(KeyCode::K, true), ms(0));
    for t in (100..600).step_by(50) {
        m.handle(&repeat(KeyCode::K), ms(t));
    }
    m.handle(&key(KeyCode::K, false), ms(600));
    m.handle(&key(KeyCode::K, true), ms(650));
    m.update(ms(650));
    assert_eq!(m.state(d).presses, 0, "650 > T after the real press");
    m.handle(&key(KeyCode::K, false), ms(700));
    // Scale change: double, not double_nearby.
    m.handle(&at(40.0, 40.0), ms(1000));
    m.handle(&mouse(L, true), ms(1000));
    m.handle(&mouse(L, false), ms(1010));
    m.rescale(ActivationTiming::new(500, 8, 8), Some((40.0, 40.0)));
    m.handle(&mouse(L, true), ms(1100));
    m.update(ms(1100));
    assert_eq!(m.state(d).presses, 1);
    assert_eq!(m.state(dn).presses, 0);
}

// ----- (s), (t) releases ------------------------------------------------------

#[test]
fn s_releases_of_buttons_not_held() {
    let (mut m, ids, ui) = defaults();
    m.handle(&mouse(L, false), ms(0));
    m.update(ms(0));
    assert_eq!(m.state(ui.tap_left), Default::default());
    m.handle(&mouse(L, true), ms(10));
    m.handle(&RawEvent::FocusLost, ms(20));
    m.handle(&mouse(L, false), ms(30));
    m.update(ms(30));
    assert_eq!(m.state(ui.tap_left).presses, 0, "no tap at focus loss");
    assert_eq!(edges(&m, ui.select_left), (1, 1, false));
    let _ = ids;
}

#[test]
fn t_release_binding_with_a_ui_band_entry() {
    let extra = [ActionDef::button("test.rel")];
    let doc = "[bindings.movement]\n\"test.rel\" = [{ input = \"F5\", on = \"release\" }]\n";
    let (mut m, _, ui) = with_extra(&extra, &[], doc);
    let rel = id(&m, "test.rel");
    m.band_push(ui.ui_dialog, 1, 0);
    let mut rec = Rec::default();
    m.handle_with(&key(KeyCode::F5, true), ms(0), &mut rec);
    m.handle_with(&key(KeyCode::F5, false), ms(900), &mut rec);
    m.update(ms(900));
    assert_eq!(edges(&m, rel), (1, 1, false));
}

// ----- (u) sequences and pushes -------------------------------------------------

#[test]
fn u_first_opaque_keys_push_drops_a_game_sequence() {
    let extra = [ActionDef::button("test.gg")];
    let doc = "[bindings.movement]\n\"test.gg\" = [\"G G\"]\n";
    for hover_only in [false, true] {
        let (mut m, _, ui) = with_extra(&extra, &[], doc);
        let gg = id(&m, "test.gg");
        m.handle(&key(KeyCode::G, true), ms(0));
        m.handle(&key(KeyCode::G, false), ms(10));
        assert!(m.sequence_pending());
        if hover_only {
            m.band_push(ui.ui_scroll_mouse, 9, 0);
        } else {
            m.activate(ui.text_entry);
        }
        assert_eq!(m.sequence_pending(), hover_only);
        m.handle(&key(KeyCode::G, true), ms(20));
        m.handle(&RawEvent::Text('g'), ms(20));
        m.update(ms(20));
        assert_eq!(m.state(gg).presses, u16::from(hover_only));
        assert_eq!(m.text(), if hover_only { "" } else { "g" });
    }
}

// ----- (v) repeat ---------------------------------------------------------------

#[test]
fn v_repeat_per_hold_command() {
    let extra = [ActionDef::button("test.x").with_repeat(true)];
    let mut reg = default_registry();
    for a in &extra {
        reg.add_action(a.clone()).unwrap();
    }
    let ui = UiActions::new(&reg).unwrap();
    let mut km = default_keymap();
    km.set_with_commands(
        "global",
        "test.x",
        vec![
            (BindingSpec::parse("Left").unwrap(), Some("CmdA".into())),
            (
                BindingSpec::digital(Chord::parse("Left").unwrap(), 0.5),
                Some("CmdB".into()),
            ),
        ],
    )
    .unwrap();
    for data in [true, false] {
        let (mut m, w) = InputMapper::new(reg.clone(), &km);
        assert!(w.is_empty());
        let g = ctx(&m, "global");
        m.activate(g);
        m.activate(ui.pointer);
        m.set_command_ids([("CmdA", 1u32), ("CmdB", 2)]);
        if data {
            m.set_repeat_commands(Some([1u32]));
        }
        let mut rec = Rec::default();
        m.handle_with(&key(KeyCode::Left, true), ms(0), &mut rec);
        m.handle_with(&repeat(KeyCode::Left), ms(500), &mut rec);
        let log = rec.log.clone();
        let repeats: Vec<_> = log
            .iter()
            .filter_map(OfferedEvent::as_action)
            .filter(|a| a.phase == OfferPhase::Repeat)
            .map(|a| (*a.command).to_owned())
            .collect();
        if data {
            assert_eq!(repeats, ["CmdA"]);
        } else {
            assert_eq!(repeats, ["CmdA", "CmdB"], "the repeat flag decides");
        }
        m.update(ms(500));
        assert_eq!(m.state(id(&m, "test.x")).presses, 1, "a Repeat is no edge");
    }
}

// ----- offered event contents -----------------------------------------------------

#[test]
fn offers_carry_command_position_and_sequence() {
    let (mut m, _, _) = defaults();
    m.set_command_ids([("SelectLeft", 0x56u32)]);
    let mut rec = Rec::default();
    m.handle_with(&at(3.0, 4.0), ms(5), &mut rec);
    m.handle_with(&key(KeyCode::ShiftLeft, true), ms(6), &mut rec);
    m.handle_with(&mouse(L, true), ms(7), &mut rec);
    let OfferedEvent::PointerMoved { time, sequence, .. } = rec.log[0] else {
        panic!("{:?}", rec.log)
    };
    assert_eq!((time, sequence), (ms(5), 2));
    let a = rec.log[1].as_action().unwrap();
    assert_eq!(&*a.command, "SelectLeft");
    assert_eq!(a.command_id, Some(0x56));
    assert_eq!(a.position, Some((3.0, 4.0)));
    assert_eq!(a.time, ms(7));
    assert_eq!(a.sequence, 3);
    assert_eq!(a.value, 1.0);
    assert!(a.modifiers.contains(tessera_input::Modifiers::SHIFT));
    // No command, no offer: fly actions go straight to game code.
    let (mut m, ids, _) = defaults();
    ids.enter_fly_mode(&mut m);
    m.handle_with(&key(KeyCode::C, true), ms(0), &mut rec);
    rec.log.clear();
    m.update(ms(0));
    assert_eq!(m.value(ids.fly_vertical), -1.0);
}

#[test]
fn focus_lost_offers_ends_then_focus_lost() {
    let (mut m, _, _) = defaults();
    let mut rec = Rec::default();
    m.handle_with(&mouse(R, true), ms(0), &mut rec);
    m.handle_with(&RawEvent::CursorLeft, ms(1), &mut rec);
    m.handle_with(&RawEvent::FocusLost, ms(2), &mut rec);
    assert_eq!(
        rec.take(&m),
        ["Start mouselook", "Left", "End mouselook", "FocusLost"]
    );
}

// ----- regressions: the band end, End order, offered values ---------------------

/// `test.k` on K in `bind_ctx`, `test.r` on the release of K in `global`.
fn release_band_end_fixture(bind_ctx: &str) -> (InputMapper, UiActions) {
    let extra = [
        ActionDef::button("test.k").with_command("TestK"),
        ActionDef::button("test.r").with_command("TestR"),
    ];
    let doc = format!(
        "[bindings.{bind_ctx}]\n\"test.k\" = [\"K\"]\n\
         [bindings.global]\n\"test.r\" = [{{ input = \"K\", on = \"release\" }}]\n"
    );
    let (m, _, ui) = with_extra(&extra, &[], &doc);
    (m, ui)
}

/// §3.3, §7.8.5: the `opaque_keys` band end of a release is fixed when the
/// release starts, like the context order, so a pop requested during the
/// release's own Ends does not let a game-band release binding fire.
#[test]
fn release_band_end_is_fixed_before_its_ends_are_offered() {
    for pop in [false, true] {
        let (mut m, ui) = release_band_end_fixture("text_entry");
        let (k, r) = (id(&m, "test.k"), id(&m, "test.r"));
        let te = ui.text_entry;
        m.band_push(te, 7, 50);
        let mut rec = Rec::default();
        if pop {
            rec.hook = Some(Box::new(move |e, req| {
                if let OfferedEvent::Action(a) = e
                    && a.action == k
                    && a.phase == OfferPhase::End
                {
                    req.pop(te, 7);
                }
            }));
        }
        m.handle_with(&key(KeyCode::K, true), ms(0), &mut rec);
        m.handle_with(&key(KeyCode::K, false), ms(900), &mut rec);
        m.update(ms(900));
        assert_eq!(
            rec.take(&m),
            ["Start test.k", "End test.k"],
            "the release stops at the band end (pop: {pop})"
        );
        assert_eq!(edges(&m, r), (0, 0, false), "pop: {pop}");
        assert_eq!(m.band().is_empty(), pop);
        // From the next raw event the popped band counts.
        if pop {
            m.handle_with(&key(KeyCode::K, true), ms(2000), &mut rec);
            m.handle_with(&key(KeyCode::K, false), ms(2100), &mut rec);
            m.update(ms(2100));
            assert_eq!(edges(&m, r), (1, 1, false));
        }
    }
}

/// The reverse: an End whose requests push the first `opaque_keys` entry
/// does not stop the release that offered it.
#[test]
fn release_band_end_ignores_a_push_during_its_ends() {
    let (mut m, ui) = release_band_end_fixture("movement");
    let (k, r) = (id(&m, "test.k"), id(&m, "test.r"));
    let te = ui.text_entry;
    let mut rec = Rec {
        hook: Some(Box::new(move |e, req| {
            if let OfferedEvent::Action(a) = e
                && a.action == k
                && a.phase == OfferPhase::End
            {
                req.push(te, 7, 50);
            }
        })),
        ..Rec::default()
    };
    m.handle_with(&key(KeyCode::K, true), ms(0), &mut rec);
    m.handle_with(&key(KeyCode::K, false), ms(900), &mut rec);
    m.update(ms(900));
    assert_eq!(
        rec.take(&m),
        ["Start test.k", "End test.k", "Start test.r", "End test.r"]
    );
    assert_eq!(edges(&m, r), (1, 1, false));
    assert_eq!(m.band().len(), 1);
}

/// §7.8.5: the Ends caused by one offer's requests are offered in the order
/// their holds began, not in request order.
#[test]
fn ends_caused_by_one_offer_follow_the_order_holds_began() {
    let (mut m, _, ui) = defaults();
    let (focus, copy) = (id(&m, "focus_next"), id(&m, "text_copy"));
    let (dialog, clipboard) = (ui.ui_dialog, ui.ui_clipboard);
    m.band_push(dialog, 1, 0);
    m.band_push(clipboard, 2, 0);
    let mut rec = Rec::consuming(&[focus, copy]);
    m.handle_with(&key(KeyCode::Tab, true), ms(0), &mut rec);
    m.handle_with(&key(KeyCode::ControlLeft, true), ms(1), &mut rec);
    m.handle_with(&key(KeyCode::C, true), ms(2), &mut rec);
    assert_eq!(rec.take(&m), ["Start focus_next", "Start text_copy"]);
    rec.hook = Some(Box::new(move |e, req| {
        if matches!(e, OfferedEvent::PointerMoved { .. }) {
            req.pop(clipboard, 2);
            req.pop(dialog, 1);
        }
    }));
    m.handle_with(&at(5.0, 5.0), ms(3), &mut rec);
    m.offer_pending(&mut rec);
    assert_eq!(
        rec.take(&m),
        ["Move 5,5", "End focus_next", "End text_copy"]
    );
}

/// §2.2, §5.3: a button action ignores the entry's `value`; its offers carry
/// 1.0.
#[test]
fn button_offers_carry_value_one() {
    for value in ["0.25", "0", "-1"] {
        let doc =
            format!("[bindings.movement]\njump = [{{ input = \"Space\", value = {value} }}]\n");
        let (mut m, ids, _) = with_extra(&[], &[], &doc);
        let mut rec = Rec::default();
        m.handle_with(&key(KeyCode::Space, true), ms(0), &mut rec);
        m.handle_with(&key(KeyCode::Space, false), ms(10), &mut rec);
        let offered: Vec<f32> = rec
            .log
            .iter()
            .filter_map(|e| match e {
                OfferedEvent::Action(a) if a.action == ids.jump => Some(a.value),
                _ => None,
            })
            .collect();
        assert_eq!(offered, [1.0, 1.0], "value = {value}");
        m.update(ms(10));
        assert_eq!(edges(&m, ids.jump), (1, 1, false));
    }
}

// ----- keeping band push (ui-widgets.md §18.4, keymap.md §3.3) ---------------

/// The client host's owner and priority of its modal `text_entry` entry.
const MODAL_OWNER: u64 = u64::MAX;

/// `band_push_keeping_holds`: game-band holds from before the push (an
/// offered key hold and a modifier hold) stay held until their own release,
/// while the entry's `opaque_keys` keeps every new key press, `toggle_mount`
/// (Ctrl+M) included, from the game band.
#[test]
fn keeping_push_leaves_holds_from_before_and_blocks_new_keys() {
    let (mut m, ids, ui) = defaults();
    let (walk, mount) = (id(&m, "walk"), id(&m, "toggle_mount"));
    let mut rec = Rec::default();
    m.handle_with(&key(KeyCode::W, true), ms(0), &mut rec);
    m.handle_with(&key(KeyCode::ShiftLeft, true), ms(0), &mut rec);
    m.update(ms(0));
    assert!(m.pressed(ids.move_forward) && m.pressed(walk));
    m.band_push_keeping_holds(ui.text_entry, MODAL_OWNER, i32::MIN);
    assert!(m.is_active(ui.text_entry));
    m.offer_pending(&mut rec);
    m.update(ms(1));
    assert!(m.pressed(ids.move_forward), "W from before still moves");
    assert!(m.pressed(walk), "Shift from before still walks");
    assert_eq!(edges(&m, ids.move_forward), (0, 0, true), "no End");
    assert!(
        !rec.take(&m).iter().any(|e| e.starts_with("End")),
        "no End offered at the push"
    );
    // A binding without a command name pressed now does not fire, while the
    // holds from before are live.
    m.handle_with(&key(KeyCode::ControlLeft, true), ms(2), &mut rec);
    m.handle_with(&key(KeyCode::M, true), ms(3), &mut rec);
    m.update(ms(3));
    assert_eq!(edges(&m, mount), (0, 0, false), "Ctrl+M under the entry");
    m.handle_with(&key(KeyCode::M, false), ms(4), &mut rec);
    m.handle_with(&key(KeyCode::ControlLeft, false), ms(4), &mut rec);
    m.update(ms(4));
    assert_eq!(edges(&m, mount), (0, 0, false));
    assert!(m.pressed(ids.move_forward) && m.pressed(walk));
    // Each hold from before ends at its own release.
    rec.take(&m);
    m.handle_with(&key(KeyCode::W, false), ms(5), &mut rec);
    m.update(ms(5));
    assert_eq!(edges(&m, ids.move_forward), (0, 1, false), "W's release");
    assert_eq!(rec.take(&m), ["End move_forward"]);
    assert!(m.pressed(walk));
    m.handle_with(&key(KeyCode::ShiftLeft, false), ms(6), &mut rec);
    m.update(ms(6));
    assert_eq!(edges(&m, walk), (0, 1, false), "Shift's release");
    // Keys pressed now reach no game-band context, modifier holds included.
    m.handle_with(&key(KeyCode::ShiftLeft, true), ms(7), &mut rec);
    m.handle_with(&key(KeyCode::W, true), ms(7), &mut rec);
    m.update(ms(7));
    assert!(!m.pressed(walk), "Shift pressed under the entry");
    assert!(!m.pressed(ids.move_forward), "W pressed under the entry");
    m.handle_with(&key(KeyCode::W, false), ms(8), &mut rec);
    m.handle_with(&key(KeyCode::ShiftLeft, false), ms(8), &mut rec);
    m.band_pop(ui.text_entry, MODAL_OWNER);
    assert!(!m.is_active(ui.text_entry));
    m.handle_with(&key(KeyCode::ControlLeft, true), ms(9), &mut rec);
    m.handle_with(&key(KeyCode::M, true), ms(10), &mut rec);
    m.update(ms(10));
    assert!(m.just_pressed(mount), "Ctrl+M works after the pop");
}

/// The same without a sink (`handle`): `toggle_mount` cannot fire under the
/// kept entry while W from before is held, and W ends at its release.
#[test]
fn keeping_push_without_a_sink() {
    let (mut m, ids, ui) = defaults();
    let mount = id(&m, "toggle_mount");
    m.handle(&key(KeyCode::W, true), ms(0));
    m.band_push_keeping_holds(ui.text_entry, MODAL_OWNER, i32::MIN);
    m.handle(&key(KeyCode::ControlLeft, true), ms(1));
    m.handle(&key(KeyCode::M, true), ms(2));
    m.update(ms(2));
    assert!(m.pressed(ids.move_forward));
    assert_eq!(edges(&m, mount), (0, 0, false));
    m.handle(&key(KeyCode::W, false), ms(3));
    m.update(ms(3));
    assert_eq!(edges(&m, ids.move_forward), (0, 1, false));
}

/// A kept entry does not count as the band's first `opaque_keys` entry: a
/// later ordinary push (a text field taking the focus) still ends the
/// game-band key holds from before and the carried modifier holds (§3.3).
#[test]
fn an_ordinary_push_after_a_kept_entry_still_ends_holds() {
    let (mut m, ids, ui) = defaults();
    let walk = id(&m, "walk");
    m.handle(&key(KeyCode::W, true), ms(0));
    m.handle(&key(KeyCode::ShiftLeft, true), ms(0));
    m.band_push_keeping_holds(ui.text_entry, MODAL_OWNER, i32::MIN);
    m.update(ms(1));
    assert!(m.pressed(ids.move_forward) && m.pressed(walk));
    m.band_push(ui.text_entry, 7, 50);
    m.update(ms(2));
    assert!(!m.pressed(ids.move_forward), "the focus push ends W");
    assert!(!m.pressed(walk), "and the modifier hold");
    m.band_pop(ui.text_entry, 7);
    m.update(ms(3));
    assert!(!m.pressed(walk), "the kept entry does not bring it back");
    // An identical push of the kept triple changes nothing.
    m.band_push(ui.text_entry, MODAL_OWNER, i32::MIN);
    assert_eq!(m.band().len(), 1);
}

/// A keeping push drops a game-band pending sequence (§3.2), so a key typed
/// under the entry cannot complete it.
#[test]
fn keeping_push_drops_a_game_band_sequence() {
    let doc = "[bindings.movement]\njump = [\"G G\"]\n";
    let (mut m, ids, ui) = with_extra(&[], &[], doc);
    m.handle(&key(KeyCode::G, true), ms(0));
    m.handle(&key(KeyCode::G, false), ms(1));
    assert!(m.sequence_pending());
    m.band_push_keeping_holds(ui.text_entry, MODAL_OWNER, i32::MIN);
    assert!(!m.sequence_pending());
    m.handle(&key(KeyCode::G, true), ms(2));
    m.update(ms(2));
    assert_eq!(edges(&m, ids.jump), (0, 0, false));
}

/// `ui-widgets.md` §18.4 pairing rule: the release of a key pressed before
/// a keeping push goes past its band end, so a game-band `release` binding
/// without a command name fires at it; a key pressed under the kept entry
/// stops there, and an ordinary `opaque_keys` entry stops both (§3.3).
#[test]
fn keeping_push_lets_the_release_of_a_key_from_before_through() {
    let extra = [ActionDef::button("test.rel")];
    let doc = "[bindings.movement]\n\"test.rel\" = [{ input = \"Insert\", on = \"release\" }]\n";
    let (mut m, _, ui) = with_extra(&extra, &[], doc);
    let rel = id(&m, "test.rel");
    let mut rec = Rec::default();
    // Pressed before the push, released under it: fires.
    m.handle_with(&key(KeyCode::Insert, true), ms(0), &mut rec);
    m.band_push_keeping_holds(ui.text_entry, MODAL_OWNER, i32::MIN);
    m.handle_with(&key(KeyCode::Insert, false), ms(500), &mut rec);
    m.update(ms(500));
    assert_eq!(m.state(rel).presses, 1, "the release from before fires");
    // Pressed under the kept entry: stops at the band end.
    m.handle_with(&key(KeyCode::Insert, true), ms(600), &mut rec);
    m.handle_with(&key(KeyCode::Insert, false), ms(1100), &mut rec);
    m.update(ms(1100));
    assert_eq!(m.state(rel).presses, 0, "pressed under the entry");
    // Pressed under the kept entry, released after the pop: fires as usual.
    m.handle_with(&key(KeyCode::Insert, true), ms(1200), &mut rec);
    m.band_pop(ui.text_entry, MODAL_OWNER);
    m.handle_with(&key(KeyCode::Insert, false), ms(1700), &mut rec);
    m.update(ms(1700));
    assert_eq!(m.state(rel).presses, 1, "no band end at the release");
    // Pressed before both, released under an ordinary entry: §3.3 stops it.
    m.handle_with(&key(KeyCode::Insert, true), ms(1800), &mut rec);
    m.band_push_keeping_holds(ui.text_entry, MODAL_OWNER, i32::MIN);
    m.band_push(ui.text_entry, 7, 50);
    m.handle_with(&key(KeyCode::Insert, false), ms(2300), &mut rec);
    m.update(ms(2300));
    assert_eq!(m.state(rel).presses, 0, "an ordinary entry stops it");
    // Pressed before a new kept push, released under it: fires.
    m.band_pop(ui.text_entry, 7);
    m.band_pop(ui.text_entry, MODAL_OWNER);
    m.handle_with(&key(KeyCode::Insert, true), ms(2400), &mut rec);
    m.band_push_keeping_holds(ui.text_entry, MODAL_OWNER, i32::MIN);
    m.handle(&key(KeyCode::Insert, false), ms(2900));
    m.update(ms(2900));
    assert_eq!(m.state(rel).presses, 1, "without a sink too");
}

/// A game-band modifier hold live only through the window system's report
/// (focus regained with Shift down: no key press) stays held across a
/// keeping push until the report drops Shift; a later report of Shift
/// holds nothing.
#[test]
fn keeping_push_carries_reported_modifiers() {
    let (mut m, _, ui) = defaults();
    let walk = id(&m, "walk");
    m.handle(&RawEvent::Modifiers(tessera_input::Modifiers::SHIFT), ms(0));
    m.update(ms(0));
    assert!(m.pressed(walk), "walking through the report");
    m.band_push_keeping_holds(ui.text_entry, MODAL_OWNER, i32::MIN);
    for t in 1..4 {
        m.update(ms(t));
        assert!(m.pressed(walk), "still walking after the push");
    }
    m.handle(
        &RawEvent::Modifiers(tessera_input::Modifiers::empty()),
        ms(5),
    );
    m.update(ms(5));
    assert!(!m.pressed(walk), "the report's release ends it");
    m.handle(&RawEvent::Modifiers(tessera_input::Modifiers::SHIFT), ms(6));
    m.update(ms(6));
    assert!(!m.pressed(walk), "Shift reported under the entry");
    m.band_pop(ui.text_entry, MODAL_OWNER);
    m.update(ms(7));
    assert!(m.pressed(walk), "after the pop the report counts again");
}

/// keymap.md §7.4: a kept-only `opaque` UI-band context stops the
/// modifier-hold evaluation of the UI-band contexts after it; the game band
/// is still evaluated with the carried modifiers (synthetic contexts).
#[test]
fn a_kept_opaque_context_stops_the_ui_modifier_holds_after_it() {
    let extra = [ActionDef::button("test.ui_shift")];
    let ctxs = [
        ContextDef::new("test.opaque", 0).ui().opaque(),
        ContextDef::new("test.below", 0).ui(),
    ];
    let doc = "[bindings.\"test.below\"]\n\"test.ui_shift\" = [\"Shift\"]\n";
    let (mut m, _, _) = with_extra(&extra, &ctxs, doc);
    let (o, y) = (ctx(&m, "test.opaque"), ctx(&m, "test.below"));
    let (walk, ui_shift) = (id(&m, "walk"), id(&m, "test.ui_shift"));
    m.band_push(y, 1, 0);
    m.handle(&key(KeyCode::ShiftLeft, true), ms(0));
    m.update(ms(0));
    assert!(m.pressed(ui_shift) && m.pressed(walk));
    m.band_push_keeping_holds(o, 2, 10);
    for t in 1..4 {
        m.update(ms(t));
        assert!(!m.pressed(ui_shift), "no hold below the opaque context");
        assert!(m.pressed(walk), "the game band keeps the carried Shift");
    }
    m.band_pop(o, 2);
    m.update(ms(5));
    assert!(m.pressed(ui_shift), "after the pop Shift holds it again");
}
