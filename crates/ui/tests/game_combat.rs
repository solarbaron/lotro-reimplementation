// SPDX-License-Identifier: GPL-3.0-or-later
//! The combat widgets (`docs/specs/combat-ui.md`) on synthetic trees: the worked examples
//! of §17, the invariants of §15, the install looks over synthetic layouts (element IDs,
//! states and properties are the placeholders of `CombatVocab::default()`) and the fallback
//! looks of §13. All values are invented.

mod ui_common;

use tessera_formats::did::Did;
use tessera_formats::property::PropertyValue;
use tessera_ui::events::{Message, MsgArg};
use tessera_ui::game::combat::cast::{CastPhase, CastTimer};
use tessera_ui::game::combat::effects::{pulse_opacity, pulses};
use tessera_ui::game::combat::floaty::{
    MAX_PENDING, base_rise_speed, distance_scale, place, text_opacity, text_scale,
};
use tessera_ui::game::combat::frames::{fallback_health_colour, fallback_name_colour};
use tessera_ui::game::combat::look::{self, procedural_image, sweep_ref, triangle_ref};
use tessera_ui::game::combat::*;
use tessera_ui::game::quickslot::{Cooldown, SkillSlots, SlotKind, SlotView};
use tessera_ui::game::{NoShortcuts, find_below};
use tessera_ui::{ElementKey, ImageRef, MemoryEnv, Ui, UiEvent, WidgetClass};
use ui_common::*;

const EPS: f32 = 1.0e-3;

fn cv() -> CombatVocab {
    CombatVocab::default()
}

fn bare_ui() -> Ui {
    ui_with(MemoryEnv::default(), 1024, 768)
}

fn fallback_ui() -> (Ui, CombatUi) {
    let mut ui = bare_ui();
    let c = CombatUi::new(
        &mut ui,
        cv(),
        CombatLayouts::default(),
        CombatRoots::default(),
        FloatySettings::default(),
    );
    (ui, c)
}

fn click(ui: &Ui, k: ElementKey, msg: u32) -> UiEvent {
    UiEvent::Game {
        message: Message {
            seq: 1,
            source: k,
            source_id: ui.element_id(k).unwrap_or(0),
            msg,
            arg: MsgArg::default(),
        },
    }
}

fn eff(instance: u64, start: f64, duration: Option<f64>) -> EffectView {
    EffectView {
        instance,
        effect_id: 0x7000_0000 + instance as u32,
        label: format!("E{instance}"),
        start,
        duration,
        ..EffectView::default()
    }
}

fn anchor(x: f32, y: f32, d: f32) -> ScreenAnchor {
    ScreenAnchor {
        x,
        y,
        distance: d,
        on_screen: true,
        drawn: true,
    }
}

// ----- data -------------------------------------------------------------------------------

#[test]
fn vital_fraction_and_text_follow_tr10() {
    assert_eq!(Vital::new(5.0, 0.0).fraction(), 0.0);
    assert_eq!(Vital::new(f64::NAN, 10.0).fraction(), 0.0);
    assert_eq!(Vital::new(20.0, 10.0).fraction(), 1.0);
    assert_eq!(Vital::new(-3.0, 10.0).fraction(), 0.0);
    assert!((Vital::new(825.0, 900.0).fraction() - 0.916_666).abs() < EPS);
    assert_eq!(Vital::new(824.5, 900.4).text(), "825/900");
    assert_eq!(Vital::new(-2.5, 0.0).text(), "-3/0");
}

// ----- effects (§8, §17.4) ----------------------------------------------------------------

#[test]
fn effect_row_order_matches_17_4() {
    let mut r = EffectRows::default();
    r.insert(eff(1, 3.4, Some(20.0)));
    r.insert(eff(2, 3.6, Some(5.0)));
    let ids = |r: &EffectRows| r.row(0).iter().map(|e| e.instance).collect::<Vec<_>>();
    assert_eq!(ids(&r), vec![2, 1]);
    r.insert(eff(3, 3.8, None));
    assert_eq!(ids(&r), vec![3, 2, 1]);
    assert!(r.satisfies_cui7());
    // B pulses at once, A from 8.4 s, C never.
    assert!(pulses(&r.row(0)[1], 3.6, false));
    assert!(!pulses(&r.row(0)[2], 8.39, false));
    assert!(pulses(&r.row(0)[2], 8.41, false));
    assert!(!pulses(&r.row(0)[0], 100.0, false));
    assert!(r.remove(2));
    assert_eq!(ids(&r), vec![3, 1]);
    assert!(!r.remove(2));
}

#[test]
fn harmful_effects_take_the_second_row_and_same_id_stays_in_order() {
    let mut r = EffectRows::default();
    let mut h = eff(1, 1.0, Some(4.0));
    h.harmful = true;
    r.insert(h);
    r.insert(eff(2, 2.0, Some(4.0)));
    let mut again = eff(3, 0.5, Some(9.0));
    again.effect_id = 0x7000_0002;
    r.insert(again);
    assert_eq!(r.row(1).len(), 1);
    // The same-ID front rule stays within CUI7 (§8.2 OPEN): the earlier start goes after.
    assert_eq!(
        r.row(0).iter().map(|e| e.instance).collect::<Vec<_>>(),
        vec![2, 3]
    );
    assert!(r.satisfies_cui7());
    // Overflow: only the first eight are shown.
    let mut many = EffectRows::default();
    for i in 0..12 {
        many.insert(eff(100 + i, f64::from(i as u32), Some(60.0)));
    }
    assert_eq!(many.shown(0, 8).len(), 8);
    assert_eq!(many.len(), 12);
    assert!(many.satisfies_cui7());
}

#[test]
fn combat_only_icons_follow_the_untimed_non_combat_group() {
    // §8.2 Order (revision 3, CU30): an untimed combat-only icon goes right after the last
    // untimed non-combat-only icon, not to position 0.
    let mut r = EffectRows::default();
    let ids = |r: &EffectRows| r.row(0).iter().map(|e| e.instance).collect::<Vec<_>>();
    let co = |instance, start, duration, combat_only| {
        let mut e = eff(instance, start, duration);
        e.combat_only = combat_only;
        e
    };
    r.insert(co(1, 1.0, Some(10.0), false)); // timed
    r.insert(co(2, 2.0, None, false)); // untimed, not combat-only
    r.insert(co(3, 3.0, None, false)); // untimed, not combat-only
    assert_eq!(ids(&r), vec![3, 2, 1]);
    // Right after the last untimed non-combat-only icon (position 2), not the front.
    r.insert(co(4, 4.0, None, true));
    assert_eq!(ids(&r), vec![3, 2, 4, 1]);
    // A fresh untimed non-combat-only icon still goes to the front, before the combat-only one.
    r.insert(co(5, 5.0, None, false));
    assert_eq!(ids(&r), vec![5, 3, 2, 4, 1]);
    // A timed combat-only icon passes the timed icons and sits after the last untimed icon.
    r.insert(co(6, 6.0, Some(1.0), true));
    assert_eq!(ids(&r), vec![5, 3, 2, 4, 6, 1]);
    assert!(r.satisfies_cui7());
}

#[test]
fn combat_only_icons_pulse_out_of_combat() {
    let mut e = eff(1, 0.0, None);
    e.combat_only = true;
    assert!(pulses(&e, 1.0, false));
    assert!(!pulses(&e, 1.0, true));
    assert!((pulse_opacity(0.0) - 1.0).abs() < EPS);
    assert!((pulse_opacity(1.0) - 0.3).abs() < EPS);
    assert!((pulse_opacity(0.5) - 0.65).abs() < EPS);
    assert!((pulse_opacity(2.0) - 1.0).abs() < EPS);
    assert!((pulse_opacity(1.5) - 0.65).abs() < EPS);
}

#[test]
fn fallback_effect_icons_are_built_ordered_and_pulsed() {
    let (mut ui, mut c) = fallback_ui();
    c.effect_applied(EffectOwner::Player, eff(1, 3.4, Some(20.0)));
    c.effect_applied(EffectOwner::Player, eff(2, 3.6, Some(5.0)));
    let mut harm = eff(9, 3.4, Some(4.0));
    harm.harmful = true;
    c.effect_applied(EffectOwner::Target, harm);
    c.update(&mut ui, 3.6, &|_| None);
    let bar = c.player_effects.as_ref().unwrap();
    assert!(bar.is_fallback());
    let a = bar.icon(1).unwrap();
    let b = bar.icon(2).unwrap();
    // B before A, 20 px icons with 2 px gaps (§13.1).
    assert_eq!(ui.layout_box(b).unwrap().l, 0);
    assert_eq!(ui.layout_box(a).unwrap().l, 22);
    assert_eq!(ui.layout_box(a).unwrap().w(), 20);
    assert_eq!(bar.effect_at(&ui, a), Some(1));
    assert!(ui.examines(a));
    // B pulses from 3.6 s: at 4.6 s its opacity is 0.3.
    c.update(&mut ui, 4.6, &|_| None);
    let op = ui.prop(b, ui.vocab().pids.object_opacity).float();
    assert!((op - 0.3).abs() < 0.01, "{op}");
    let op_a = ui.prop(a, ui.vocab().pids.object_opacity).float();
    assert!((op_a - 1.0).abs() < EPS);
    assert!(c.effect_removed(EffectOwner::Player, 2));
    c.update(&mut ui, 4.7, &|_| None);
    let bar = c.player_effects.as_ref().unwrap();
    assert!(bar.icon(2).is_none());
    assert!(!ui.exists(b));
    assert_eq!(ui.layout_box(bar.icon(1).unwrap()).unwrap().l, 0);
    // The target row: harmful in the second row element.
    let t = c.target_effects.as_ref().unwrap();
    let rows = t.row_elements();
    assert_eq!(ui.parent(t.icon(9).unwrap()), Some(rows[1]));
}

// ----- floating text (§6, §17.2) ----------------------------------------------------------

#[test]
fn floating_text_numbers_match_17_2() {
    assert!((base_rise_speed(768.0) - 64.0).abs() < EPS);
    let s = distance_scale(10.0, 4.0, 30.0, 1.2, 0.5);
    assert!((s - 0.9142).abs() < 1.0e-3, "{s}");
    assert!((distance_scale(2.0, 4.0, 30.0, 1.2, 0.5) - 1.2).abs() < EPS);
    assert!((distance_scale(40.0, 4.0, 30.0, 1.2, 0.5) - 0.5).abs() < EPS);
    assert!((text_scale(0.25, 1.5, false, 0.06, 1.7) - 1.25).abs() < EPS);
    assert!((text_scale(0.75, 1.5, false, 0.06, 1.7) - 1.5).abs() < EPS);
    assert!((text_scale(0.03, 1.5, true, 0.06, 1.7) - 2.025).abs() < EPS);
    assert!((text_scale(0.06, 1.5, true, 0.06, 1.7) - 1.5).abs() < EPS);
    assert!((text_opacity(0.25) - 1.0).abs() < EPS);
    assert!((text_opacity(0.75) - 0.5).abs() < EPS);
    assert!(text_opacity(1.0).abs() < EPS);
    // Placement: centred, bottom at the anchor plus the offset (× σ).
    let (l, t, w, h) = place(
        (500.0, 300.0),
        (100.0, 20.0),
        1.0,
        1.0,
        (0.0, -10.0),
        (1024, 768),
    );
    assert!((w - 100.0).abs() < EPS && (h - 20.0).abs() < EPS);
    assert!((l - 450.0).abs() < EPS && (t - 270.0).abs() < EPS);
}

#[test]
fn tr19_texts() {
    assert_eq!(
        FloatyText::damage(209.5, true, false).unwrap().text,
        "\u{2212}210"
    );
    assert!(FloatyText::damage(0.2, false, false).is_none());
    assert_eq!(
        FloatyText::damage(75.0, false, true).unwrap().colour,
        0xFFFF_4040
    );
    assert_eq!(FloatyText::heal(60.0, false).unwrap().text, "+60");
    assert_eq!(FloatyText::avoidance("Parry").colour, 0xFFFF_FFFF);
}

#[test]
fn floating_text_lives_rises_and_expires() {
    let (mut ui, mut c) = fallback_ui();
    let a = |_e: EntityId| Some(anchor(512.0, 400.0, 10.0));
    assert!(c.floating_text(7, FloatyText::new("-210", 0xFFF7_E4A9, false)));
    c.update(&mut ui, 0.0, &a);
    let f = c.floaty.as_ref().unwrap();
    assert_eq!(f.counts(7), (1, 0));
    assert_eq!(f.shown().len(), 1);
    let b0 = f.shown()[0].bx;
    c.update(&mut ui, 1.5, &a);
    let f = c.floaty.as_ref().unwrap();
    let s = &f.shown()[0];
    assert!((s.opacity - 0.5).abs() < EPS);
    assert!(s.bx.1 < b0.1, "the text rises");
    c.update(&mut ui, 2.0, &a);
    assert!(
        c.floaty.as_ref().unwrap().shown().is_empty(),
        "hidden at t = 1"
    );
    assert_eq!(c.floaty.as_ref().unwrap().counts(7), (1, 0));
    c.update(&mut ui, 2.1, &a);
    assert_eq!(
        c.floaty.as_ref().unwrap().counts(7),
        (0, 0),
        "removed at t = 1.05"
    );
}

#[test]
fn pending_texts_wait_and_are_capped() {
    let (mut ui, mut c) = fallback_ui();
    let a = |_e: EntityId| Some(anchor(512.0, 400.0, 10.0));
    for i in 0..60 {
        let ok = c.floating_text(3, FloatyText::new(format!("{i}"), 0xFFFF_FFFF, false));
        assert_eq!(ok, i <= MAX_PENDING, "text {i}");
    }
    // CUI3: never more than 51 pending.
    assert_eq!(c.floaty.as_ref().unwrap().counts(3), (0, MAX_PENDING + 1));
    c.update(&mut ui, 0.0, &a);
    assert_eq!(c.floaty.as_ref().unwrap().counts(3).0, 1);
    // The next one waits until the first has risen (1 − 0.35) × 22 px ≈ 14.3 px.
    c.update(&mut ui, 0.05, &a);
    assert_eq!(c.floaty.as_ref().unwrap().counts(3).0, 1);
    let mut t = 0.05;
    while c.floaty.as_ref().unwrap().counts(3).0 < 2 && t < 2.0 {
        t += 0.05;
        c.update(&mut ui, t, &a);
    }
    assert!(t > 0.1 && t < 0.5, "released at {t}");
}

#[test]
fn critical_texts_take_slots_and_do_not_rise() {
    let (mut ui, mut c) = fallback_ui();
    c.set_player_entity(Some(1));
    let a = |_e: EntityId| Some(anchor(512.0, 400.0, 2.0));
    c.floating_text(5, FloatyText::new("-1", 0xFFFF_FFFF, true));
    c.floating_text(5, FloatyText::new("-2", 0xFFFF_FFFF, true));
    c.floating_text(1, FloatyText::new("-3", 0xFFFF_FFFF, true));
    c.update(&mut ui, 0.0, &a);
    c.update(&mut ui, 0.01, &a);
    let f = c.floaty.as_ref().unwrap();
    assert_eq!(f.slots_taken(5), 2);
    assert_eq!(
        f.slots_taken(1),
        0,
        "a critical on the local player is a normal text"
    );
    let first = f.shown().iter().find(|s| s.text == "-1").unwrap().clone();
    let y0 = first.bx.1;
    c.update(&mut ui, 1.0, &a);
    let f = c.floaty.as_ref().unwrap();
    let s = f.shown().iter().find(|s| s.text == "-1").unwrap();
    // Height shrinks back to s1 but the bottom edge does not rise.
    assert!((s.bx.1 + s.bx.3 - (y0 + first.bx.3)).abs() < 1.0);
    // The second critical sits one slot to the left (−60 × σ).
    let second = f.shown().iter().find(|s| s.text == "-2").unwrap();
    let cx1 = s.bx.0 + s.bx.2 / 2.0;
    let cx2 = second.bx.0 + second.bx.2 / 2.0;
    assert!((cx1 - cx2 - 60.0 * 1.2).abs() < 1.0, "{cx1} {cx2}");
}

#[test]
fn texts_without_anchor_are_hidden() {
    let (mut ui, mut c) = fallback_ui();
    c.floating_text(9, FloatyText::new("x", 0xFFFF_FFFF, false));
    c.update(&mut ui, 0.0, &|_| None);
    assert!(c.floaty.as_ref().unwrap().shown().is_empty());
    let mut off = anchor(0.0, 0.0, 5.0);
    off.on_screen = false;
    c.update(&mut ui, 0.1, &move |_| Some(off));
    assert!(c.floaty.as_ref().unwrap().shown().is_empty());
}

// ----- cast bars (§7) ---------------------------------------------------------------------

fn cast(d: f64) -> CastView {
    CastView {
        label: "Jab".into(),
        interruptible: true,
        cancellable: true,
        duration: d,
    }
}

#[test]
fn cast_timer_fill_delay_and_end() {
    let mut t = CastTimer::new(cast(0.8), 2.2);
    assert!((t.position(2.5) - 0.375).abs() < EPS);
    t.delay(0.4);
    assert!((t.position(2.5) - 0.25).abs() < EPS);
    t.end(CastOutcome::Interrupted, 2.5);
    assert_eq!(t.phase, CastPhase::Interrupted);
    assert!((t.position(9.0) - 0.25).abs() < EPS);
    assert!(!t.expired(3.4));
    assert!(t.expired(3.5));
    let mut t = CastTimer::new(cast(0.8), 2.2);
    t.end(CastOutcome::Completed, 3.0);
    assert_eq!(t.position(3.0), 1.0);
    assert_eq!(CastTimer::new(cast(0.0), 1.0).position(1.0), 1.0);
    assert!(CastTimer::new(cast(f64::INFINITY), 1.0).position(2.0) <= 1.0);
}

#[test]
fn fallback_cast_bar_fills_ends_and_cancels_on_click() {
    let (mut ui, mut c) = fallback_ui();
    c.cast_started(&mut ui, CastOwner::Player, &cast(0.8), 2.2);
    c.update(&mut ui, 2.5, &|_| None);
    let bar = c.player_cast.as_ref().unwrap();
    assert!(bar.is_fallback());
    let root = bar.root();
    assert!(ui.is_visible(root));
    assert_eq!(c.escape_action(), Some(CombatAction::CancelCast));
    // A click on the bar.
    let ev = click(&ui, root, ui.vocab().msgs.mouse_click);
    assert_eq!(c.handle_event(&mut ui, &ev), vec![CombatAction::CancelCast]);
    c.cast_ended(&mut ui, CastOwner::Player, CastOutcome::Completed, 3.0);
    c.update(&mut ui, 3.5, &|_| None);
    assert!(ui.is_visible(root));
    assert_eq!(
        c.handle_event(&mut ui, &ev),
        vec![],
        "no cancel after the end"
    );
    c.update(&mut ui, 4.0, &|_| None);
    assert!(c.player_cast.as_ref().unwrap().cast().is_none());
    assert_eq!(c.escape_action(), None);
}

#[test]
fn escape_prefers_cancel_then_clear() {
    let (mut ui, mut c) = fallback_ui();
    let mut craft = cast(5.0);
    craft.cancellable = false;
    c.cast_started(&mut ui, CastOwner::Player, &craft, 0.0);
    assert_eq!(c.escape_action(), None);
    let v = FrameView {
        entity: 4,
        name: "Grey boar".into(),
        has_vitals: true,
        ..FrameView::default()
    };
    c.set_target(&mut ui, Some(&v), Some(SelectionView::default()));
    assert_eq!(c.escape_action(), Some(CombatAction::ClearSelection));
}

// ----- frames (§4, §5, §13.1) -------------------------------------------------------------

#[test]
fn fallback_frames_show_bars_and_fields() {
    let (mut ui, mut c) = fallback_ui();
    let p = FrameView {
        entity: 1,
        name: "Tamsin".into(),
        level: LevelShown::Level(12),
        has_vitals: true,
        health: Some(Vital::new(450.0, 900.0)),
        power: Some(Vital::new(400.0, 400.0)),
        resource: Some(ClassResource::Plain(Vital::new(2.0, 5.0))),
        ..FrameView::default()
    };
    c.set_player(&mut ui, &p);
    let f = c.player.as_ref().unwrap();
    assert!(f.is_fallback());
    assert!(ui.is_visible(f.root()));
    // The target frame is hidden without a selection (TR17).
    assert!(!ui.is_visible(c.target.as_ref().unwrap().root()));
    let post = FrameView {
        entity: 3,
        name: "Signpost".into(),
        has_vitals: false,
        name_colour: Some(NameColour::Object),
        ..FrameView::default()
    };
    c.set_target(&mut ui, Some(&post), None);
    assert!(ui.is_visible(c.target.as_ref().unwrap().root()));
    let boar = FrameView {
        entity: 2,
        name: "Grey boar".into(),
        has_vitals: true,
        name_colour: Some(NameColour::Con(ConRank::Tough)),
        health: Some(Vital::new(990.0, 1200.0)),
        health_colour: HealthColour::Red,
        level: LevelShown::Level(7),
        ..FrameView::default()
    };
    c.set_target(&mut ui, Some(&boar), Some(SelectionView::default()));
    assert_eq!(c.selection(), Some(2));
    c.set_selection_target(&mut ui, Some(&p));
    assert!(ui.is_visible(c.selection_target.as_ref().unwrap().root()));
    c.set_target(&mut ui, None, None);
    assert!(!ui.is_visible(c.target.as_ref().unwrap().root()));
    assert!(!ui.is_visible(c.selection_target.as_ref().unwrap().root()));
    assert_eq!(
        fallback_name_colour(NameColour::Con(ConRank::Tough)),
        0xFFE0_D020
    );
    assert_eq!(fallback_health_colour(HealthColour::Grey), 0xFF80_8080);
}

#[test]
fn fallback_bar_widths_follow_the_fraction() {
    let mut ui = bare_ui();
    let mut hud = look::Hud::default();
    let h = hud.root(&mut ui).unwrap();
    let b = look::Bar::new(
        &mut ui,
        h,
        tessera_ui::IBox::from_xywh(0, 0, 190, 10),
        1,
        2,
        true,
    )
    .unwrap();
    b.set(&mut ui, 0.5);
    let fb = ui.layout_box(b.fill).unwrap();
    assert_eq!((fb.l, fb.w()), (95, 95));
    b.set(&mut ui, 2.0);
    assert_eq!(ui.layout_box(b.fill).unwrap().w(), 190);
    b.set(&mut ui, f32::NAN);
    assert_eq!(ui.layout_box(b.fill).unwrap().w(), 0);
    assert_eq!(look::hud_transform((1920, 1080)), (1.40625, 240));
}

const L: u32 = 0x22FF_7000;

fn avatar_layout() -> tessera_formats::ui_layout::UiLayout {
    let i = cv().ids;
    let s = cv().states;
    let vv = v();
    let meter = |id: u32| typed(id, vv.types.meter, 0, 0, 164, 9);
    let mut root = field(i.avatar_panel, 0, 0, 280, 300);
    root = child(root, typed(i.player_name, vv.types.text, 87, 3, 191, 36));
    root = child(root, typed(i.player_level, vv.types.text, 32, 82, 38, 17));
    root = child(root, meter(i.health_meter));
    root = child(
        root,
        child(field(i.power_field, 0, 17, 164, 9), meter(i.power_meter)),
    );
    root = child(
        root,
        child(field(i.rage_field, 1, 17, 164, 9), meter(i.rage_meter)),
    );
    root = child(root, typed(i.health_text, vv.types.text, 0, 8, 162, 16));
    root = child(
        root,
        state(
            field(i.avatar_bg, -1, 0, 273, 124),
            s.normal,
            0,
            vec![],
            vec![],
        ),
    );
    let mut opp = field(i.opponent_panel, 300, 0, 305, 300);
    let mut disp = field(i.opponent_display, 0, 20, 302, 300);
    disp = child(disp, typed(i.opponent_name, vv.types.text, 2, 3, 216, 36));
    disp = child(
        disp,
        typed(i.opponent_level, vv.types.text, 232, 82, 38, 17),
    );
    disp = child(disp, field(i.opponent_skull, 220, 79, 65, 32));
    disp = child(
        disp,
        state(
            typed(i.opponent_health_meter, vv.types.meter, 44, 45, 164, 9),
            s.meter_green,
            0,
            vec![],
            vec![],
        ),
    );
    opp = child(opp, disp);
    let mut inan = field(i.inanimate_field, 40, 1, 228, 84);
    inan = child(
        inan,
        typed(i.inanimate_name, vv.types.text, 11, 29, 205, 47),
    );
    opp = child(opp, inan);
    opp = child(opp, field(i.opponent_effects, 37, 74, 170, 200));
    let mut bar = field(i.progress_bar, 32, 28, 255, 26);
    bar = state(bar, s.cast_init, 0, vec![], vec![]);
    bar = state(bar, s.cast_succeeded, 0, vec![], vec![]);
    bar = child(bar, typed(i.progress_meter, vv.types.meter, 12, 6, 234, 13));
    bar = child(bar, typed(i.progress_label, vv.types.text, 11, 4, 238, 16));
    let dragbox = child(field(i.progress_dragbox, 380, 598, 320, 66), bar);
    let icon = child(
        child(
            field(i.effect_icon, 0, 0, 36, 36),
            field(i.effect_border, 0, 0, 36, 36),
        ),
        field(i.effect_image, 2, 2, 32, 32),
    );
    layout(L, 1024, 768, vec![root, opp, dragbox, icon])
}

#[test]
fn install_frames_set_meters_texts_and_states() {
    let mut ui = ui_with(env_with(vec![avatar_layout()]), 1024, 768);
    let c = cv();
    let avatar = ui.open_panel(Did(L), c.ids.avatar_panel).unwrap();
    let opp = ui.open_panel(Did(L), c.ids.opponent_panel).unwrap();
    let progress = ui.open_panel(Did(L), c.ids.progress_dragbox).unwrap();
    let mut combat = CombatUi::new(
        &mut ui,
        c,
        CombatLayouts {
            effects: Some(Did(L)),
            ..CombatLayouts::default()
        },
        CombatRoots {
            avatar_panel: Some(avatar),
            opponent_panel: Some(opp),
            player_progress: Some(progress),
            ..CombatRoots::default()
        },
        FloatySettings::default(),
    );
    assert!(!combat.player.as_ref().unwrap().is_fallback());
    assert!(!combat.target.as_ref().unwrap().is_fallback());
    assert!(combat.selection_target.as_ref().unwrap().is_fallback());
    assert!(!combat.player_cast.as_ref().unwrap().is_fallback());
    assert!(!combat.target_effects.as_ref().unwrap().is_fallback());
    let mp = ui.vocab().pids.meter_position;
    let p = FrameView {
        entity: 1,
        name: "Tamsin".into(),
        level: LevelShown::Level(12),
        has_vitals: true,
        health: Some(Vital::new(450.0, 900.0)),
        power: Some(Vital::new(100.0, 400.0)),
        resource: Some(ClassResource::Wrath(Vital::new(30.0, 100.0))),
        ..FrameView::default()
    };
    combat.set_player(&mut ui, &p);
    let f = |id: u32, ui: &Ui, root| find_below(ui, root, id).unwrap();
    assert!((ui.prop(f(c.ids.health_meter, &ui, avatar), mp).float() - 0.5).abs() < EPS);
    assert!((ui.prop(f(c.ids.power_meter, &ui, avatar), mp).float() - 0.25).abs() < EPS);
    assert!((ui.prop(f(c.ids.rage_meter, &ui, avatar), mp).float() - 0.3).abs() < EPS);
    assert!(
        !ui.is_visible(f(c.ids.power_field, &ui, avatar)),
        "wrath replaces power"
    );
    assert!(ui.is_visible(f(c.ids.rage_field, &ui, avatar)));
    let name = f(c.ids.player_name, &ui, avatar);
    assert_eq!(ui.text_of(name), "Tamsin");
    assert_eq!(ui.text_of(f(c.ids.player_level, &ui, avatar)), "12");
    assert_eq!(
        ui.state(f(c.ids.avatar_bg, &ui, avatar)),
        Some(c.states.normal)
    );
    assert!(
        combat.resource_bar().is_none(),
        "the wrath meter shows wrath"
    );
    // A resource without a pip record takes the §13.4 bar beside the install frame (TR11,
    // TR41), and goes away when the resource changes.
    let mut plain = p.clone();
    plain.resource = Some(ClassResource::Plain(Vital::new(2.0, 5.0)));
    combat.set_player(&mut ui, &plain);
    let rb = combat.resource_bar().expect("the §13.4 bar");
    assert!(ui.is_visible(rb));
    combat.set_player(&mut ui, &p);
    assert!(!ui.is_visible(rb));
    // Vital texts show only under the pointer (TR10).
    let ht = f(c.ids.health_text, &ui, avatar);
    combat.update(&mut ui, 0.0, &|_| None);
    assert!(!ui.is_visible(ht));
    // Target: vital field vs non-vital field (§5.2 step 1), colour states, skull.
    let mut t = FrameView {
        entity: 2,
        name: "Signpost".into(),
        has_vitals: false,
        ..FrameView::default()
    };
    combat.set_target(&mut ui, Some(&t), None);
    assert!(ui.is_visible(opp));
    assert!(!ui.is_visible(f(c.ids.opponent_display, &ui, opp)));
    assert_eq!(ui.text_of(f(c.ids.inanimate_name, &ui, opp)), "Signpost");
    t.has_vitals = true;
    t.name = "Boar".into();
    t.health = Some(Vital::new(1.0, 4.0));
    t.health_colour = HealthColour::Green;
    t.level = LevelShown::Skull;
    combat.set_target(&mut ui, Some(&t), None);
    assert!(ui.is_visible(f(c.ids.opponent_display, &ui, opp)));
    assert!(!ui.is_visible(f(c.ids.inanimate_field, &ui, opp)));
    let hm = f(c.ids.opponent_health_meter, &ui, opp);
    assert!((ui.prop(hm, mp).float() - 0.25).abs() < EPS);
    assert_eq!(ui.state(hm), Some(c.states.meter_green));
    assert!(ui.is_visible(f(c.ids.opponent_skull, &ui, opp)));
    assert!(!ui.is_visible(f(c.ids.opponent_level, &ui, opp)));
    // The install cast bar.
    combat.cast_started(&mut ui, CastOwner::Player, &cast(1.0), 0.0);
    combat.update(&mut ui, 0.5, &|_| None);
    let bar = f(c.ids.progress_bar, &ui, progress);
    assert!(ui.is_visible(bar));
    assert_eq!(ui.state(bar), Some(c.states.cast_init));
    let pm = f(c.ids.progress_meter, &ui, progress);
    assert!((ui.prop(pm, mp).float() - 0.5).abs() < EPS);
    assert_eq!(ui.text_of(f(c.ids.progress_label, &ui, progress)), "Jab");
    combat.cast_ended(&mut ui, CastOwner::Player, CastOutcome::Completed, 0.6);
    assert_eq!(ui.state(bar), Some(c.states.cast_succeeded));
    assert!((ui.prop(pm, mp).float() - 1.0).abs() < EPS);
    // Install effect icons in the target's list; an effect without an icon (a literal)
    // takes the §13.5 fallback box element by element (TR41).
    let mut e = eff(5, 0.0, Some(30.0));
    e.harmful = true;
    e.icon = Some(tessera_ui::game::IconImage::Composed(ImageRef::Did(
        0x4100_0005,
    )));
    combat.effect_applied(EffectOwner::Target, e);
    let mut lit = eff(6, 0.0, Some(30.0));
    lit.harmful = true;
    combat.effect_applied(EffectOwner::Target, lit);
    combat.update(&mut ui, 0.7, &|_| None);
    let te = combat.target_effects.as_ref().unwrap();
    let literal = te.icon(6).unwrap();
    assert_ne!(ui.element_id(literal), Some(c.ids.effect_icon));
    assert!(
        ui.is_visible(literal),
        "the literal effect's fallback box is shown"
    );
    let icon = te.icon(5).unwrap();
    assert_eq!(ui.element_id(icon), Some(c.ids.effect_icon));
    assert_eq!(ui.layout_box(icon).unwrap().w(), 20);
    let border = find_below(&ui, icon, c.ids.effect_border).unwrap();
    assert!(!ui.is_visible(border), "no border without a category");
    let image = find_below(&ui, icon, c.ids.effect_image).unwrap();
    assert_eq!(
        ui.prop(image, c.pids.tooltip_effect_instance).value(),
        Some(&PropertyValue::Int64(5))
    );
    assert_eq!(combat.effect_at(&ui, image), Some((EffectOwner::Target, 5)));
}

#[test]
fn pip_displays_show_the_matching_type() {
    let c = cv();
    let vv = v();
    let pip_ty = tessera_ui::game::GameVocab::default().types.pip_display;
    let entry = |value: i32, st: u32| {
        strukt(vec![
            (0xFFFD_9001, PropertyValue::Int(value)),
            (0xFFFD_9002, PropertyValue::Enum(st)),
        ])
    };
    let mut a = typed(0x1000_0901, pip_ty, 0, 0, 10, 10);
    a = prop(a, c.pids.pip_type, en(1));
    a = prop(
        a,
        c.pids.pip_list,
        array(
            0xFFFD_9000,
            vec![
                entry(0, 0xFFFA_9000),
                entry(2, 0xFFFA_9002),
                entry(4, 0xFFFA_9004),
            ],
        ),
    );
    for s in [0xFFFA_9000, 0xFFFA_9002, 0xFFFA_9004] {
        a = state(a, s, 0, vec![], vec![]);
    }
    let mut b = typed(0x1000_0902, pip_ty, 0, 0, 10, 10);
    b = prop(b, c.pids.pip_type, en(2));
    let root = child(child(field(0x1000_0900, 0, 0, 100, 100), a), b);
    let mut ui = ui_with(env_with(vec![layout(L, 1024, 768, vec![root])]), 1024, 768);
    let _ = vv;
    let r = ui.open_panel(Did(L), 0x1000_0900).unwrap();
    let res = ClassResource::Pips {
        pip_type: 1,
        value: Vital::new(3.0, 5.0),
    };
    let shown = frames::show_pips(&mut ui, &c, r, pip_ty, Some(&res)).unwrap();
    assert_eq!(ui.element_id(shown), Some(0x1000_0901));
    assert_eq!(ui.state(shown), Some(0xFFFA_9002), "nearest lower value");
    let other = find_below(&ui, r, 0x1000_0902).unwrap();
    assert!(!ui.is_visible(other));
    assert!(frames::show_pips(&mut ui, &c, r, pip_ty, None).is_none());
    assert!(!ui.is_visible(shown));
}

// ----- combat state, defeat (§10) ---------------------------------------------------------

#[test]
fn combat_state_and_defeat_panel_fallbacks() {
    let (mut ui, mut c) = fallback_ui();
    let ind = c.combat_state.as_ref().unwrap().indicator().unwrap();
    assert!(!ui.is_visible(ind));
    c.set_in_combat(&mut ui, true);
    assert!(ui.is_visible(ind));
    assert!(c.in_combat());
    c.set_in_combat(&mut ui, false);
    assert!(!ui.is_visible(ind));
    assert!(c.player_defeated(&mut ui));
    let panel = c.defeat.as_ref().unwrap().panel().unwrap();
    assert_eq!(ui.layout_box(panel).unwrap().w(), 512);
    // The OK button closes it.
    let ok = tessera_ui::game::subtree(&ui, panel)
        .into_iter()
        .find(|k| {
            ui.element_id(*k)
                .is_some_and(|id| (0x7EF0_0000..0x7EFF_0000).contains(&id))
        })
        .unwrap();
    let ev = click(&ui, ok, ui.vocab().msgs.mouse_click);
    assert_eq!(
        c.handle_event(&mut ui, &ev),
        vec![CombatAction::DefeatAcknowledged]
    );
    assert!(!ui.exists(panel));
    assert!(c.player_defeated(&mut ui));
    c.player_revived(&mut ui);
    assert!(c.defeat.as_ref().unwrap().panel().is_none());
}

const DIALOG: u32 = 0x22FF_7100;

/// A synthetic `Dialog` layout with the defeat panel of §10.2/CU29: the box's art first,
/// then the statement, the two listed buttons, the paid-revive and store buttons, the
/// divider, the paper-item name label and balance (each by its real label, TR38a), and —
/// to prove the position-based reading is gone (TR38a) — a piece of box art placed **after**
/// the statement's holder in child order, which must keep the layout's visibility.
fn dialog_layout() -> tessera_formats::ui_layout::UiLayout {
    let i = cv().ids;
    let vv = v();
    let mut bx = field(i.dialog_box, 0, 0, 512, 220);
    bx = child(bx, field(0x1000_7101, 0, 0, 36, 36));
    bx = child(bx, field(0x1000_7102, 7, 6, 499, 207));
    bx = child(
        bx,
        child(
            field(0x1000_7103, 10, 8, 493, 122),
            typed(i.dialog_statement, vv.types.text, 0, 0, 493, 122),
        ),
    );
    bx = child(bx, typed(i.dialog_ok, vv.types.button, 321, 163, 170, 36));
    bx = child(
        bx,
        state(
            typed(i.dialog_revive, vv.types.button, 137, 163, 170, 36),
            vv.states.ghosted,
            0,
            vec![],
            vec![],
        ),
    );
    bx = child(bx, typed(i.dialog_pay, vv.types.button, 137, 163, 170, 36));
    bx = child(bx, field(i.dialog_buy, 137, 163, 170, 36));
    bx = child(bx, field(i.dialog_divider, 57, 132, 400, 26));
    bx = child(bx, field(i.dialog_paper_name, 28, 150, 200, 16));
    bx = child(
        bx,
        prop(
            child(
                field(i.dialog_paper_balance, 28, 168, 87, 33),
                typed(0x1000_7106, vv.types.button, 0, 0, 87, 33),
            ),
            cv().pids.paper_item_service,
            int(1),
        ),
    );
    // Box art after the statement in child order: TR38a keeps it (the old position rule hid it).
    bx = child(bx, field(0x1000_7107, 0, 0, 512, 220));
    let root = child(field(i.awaken_dialog, 256, 274, 512, 220), bx);
    layout(DIALOG, 1024, 768, vec![root])
}

#[test]
fn install_defeat_panel_shows_only_tr38_parts() {
    let mut ui = ui_with(env_with(vec![dialog_layout()]), 1024, 768);
    let c = cv();
    let mut d = DefeatPanel::install(&c, Did(DIALOG));
    assert!(d.open(&mut ui));
    let panel = d.panel().unwrap();
    let f = |id: u32, ui: &Ui| find_below(ui, panel, id).unwrap();
    // Shown: the box's art (before and after the statement in child order, TR38a), the
    // statement (Tessera's own text), OK and the ghosted revive.
    for id in [
        0x1000_7101,
        0x1000_7102,
        0x1000_7107,
        c.ids.dialog_statement,
    ] {
        assert!(ui.is_displayed(f(id, &ui)), "{id:#x} shown");
    }
    assert_eq!(
        ui.text_of(f(c.ids.dialog_statement, &ui)),
        tessera_ui::game::combat::state::DEFEAT_STATEMENT
    );
    let (ok, revive) = d.buttons();
    assert_eq!(ok, Some(f(c.ids.dialog_ok, &ui)));
    assert_eq!(revive, Some(f(c.ids.dialog_revive, &ui)));
    let revive = revive.unwrap();
    assert!(ui.is_displayed(ok.unwrap()) && ui.is_displayed(revive));
    let ghosted = ui.vocab().pids.button_ghosted;
    assert!(ui.prop(revive, ghosted).bool(), "TR38: revive ghosted");
    assert_eq!(ui.state(revive), Some(ui.vocab().states.ghosted));
    // Hidden by label (TR38a): paid revive, store, divider, paper-item name and balance; the
    // balance's child (under the hidden `UI_PaperItem_PaperItemService` holder) too.
    for id in [
        c.ids.dialog_pay,
        c.ids.dialog_buy,
        c.ids.dialog_divider,
        c.ids.dialog_paper_name,
        c.ids.dialog_paper_balance,
        0x1000_7106,
    ] {
        assert!(!ui.is_displayed(f(id, &ui)), "{id:#x} hidden");
    }
    let buttons = tessera_ui::game::subtree(&ui, panel)
        .into_iter()
        .filter(|k| ui.class(*k) == Some(WidgetClass::Button) && ui.is_displayed(*k))
        .count();
    assert_eq!(buttons, 2);
    d.close(&mut ui);
    assert!(d.panel().is_none() && d.buttons() == (None, None));
}

#[test]
fn property_display_frames_draw_name_and_level_as_text() {
    let c = cv();
    let i = c.ids;
    let ty = c.types.property_display;
    let mut root = field(i.avatar_panel, 0, 0, 280, 300);
    root = child(root, typed(i.player_name, ty, 87, 3, 191, 36));
    root = child(
        root,
        child(
            field(0x1000_7201, 32, 82, 38, 17),
            typed(i.player_level, ty, 0, 0, 38, 17),
        ),
    );
    let mut opp = field(i.opponent_panel, 300, 0, 305, 300);
    let mut disp = field(i.opponent_display, 0, 20, 302, 300);
    disp = child(disp, typed(i.opponent_name, v().types.text, 2, 3, 216, 36));
    disp = child(disp, typed(i.opponent_level, ty, 232, 82, 38, 17));
    opp = child(opp, disp);
    let mut ui = ui_with(
        env_with(vec![layout(L, 1024, 768, vec![root, opp])]),
        1024,
        768,
    );
    // Registered before the panels are built (the UI host does it at start).
    c.register_classes(&mut ui);
    let avatar = ui.open_panel(Did(L), i.avatar_panel).unwrap();
    let target = ui.open_panel(Did(L), i.opponent_panel).unwrap();
    let mut combat = CombatUi::new(
        &mut ui,
        c,
        CombatLayouts::default(),
        CombatRoots {
            avatar_panel: Some(avatar),
            opponent_panel: Some(target),
            ..CombatRoots::default()
        },
        FloatySettings::default(),
    );
    let f = |id: u32, ui: &Ui, root| find_below(ui, root, id).unwrap();
    let (name, level) = (
        f(i.player_name, &ui, avatar),
        f(i.player_level, &ui, avatar),
    );
    assert!(ui.class(name).is_some_and(WidgetClass::is_text));
    assert!(ui.class(level).is_some_and(WidgetClass::is_text));
    assert_eq!(ui.text_of(level), "", "no level before the frame has one");
    combat.set_player(
        &mut ui,
        &FrameView {
            entity: 1,
            name: "Tamsin".into(),
            level: LevelShown::Level(20),
            has_vitals: true,
            ..FrameView::default()
        },
    );
    assert_eq!(ui.text_of(name), "Tamsin");
    assert_eq!(ui.text_of(level), "20");
    assert!(ui.is_displayed(name) && ui.is_displayed(level));
    assert_eq!(
        combat.player.as_ref().unwrap().name_and_level(),
        (Some(name), Some(level))
    );
    let ol = f(i.opponent_level, &ui, target);
    let mut t = FrameView {
        entity: 2,
        name: "Boar".into(),
        level: LevelShown::Level(7),
        has_vitals: true,
        ..FrameView::default()
    };
    combat.set_target(&mut ui, Some(&t), None);
    assert!(ui.class(ol).is_some_and(WidgetClass::is_text));
    assert_eq!(ui.text_of(ol), "7");
    assert!(ui.is_displayed(ol));
    // TR16: without a reported level the element shows nothing.
    t.level = LevelShown::Nothing;
    combat.set_target(&mut ui, Some(&t), None);
    assert_eq!(ui.text_of(ol), "");
}

// ----- selection display (§3.5, §13.7) ----------------------------------------------------

#[test]
fn selection_marker_follows_the_anchor() {
    let (mut ui, mut c) = fallback_ui();
    let boar = FrameView {
        entity: 2,
        name: "Grey boar".into(),
        has_vitals: true,
        ..FrameView::default()
    };
    let view = SelectionView {
        name: "Grey boar".into(),
        colour: Some(NameColour::Con(ConRank::Even)),
        health: Some(Vital::new(1.0, 2.0)),
        ..SelectionView::default()
    };
    c.set_target(&mut ui, Some(&boar), Some(view));
    c.update(&mut ui, 0.0, &|e| {
        (e == 2).then(|| anchor(400.0, 300.0, 4.0))
    });
    let (block, ind) = c.marker.as_ref().unwrap().elements();
    let (block, ind) = (block.unwrap(), ind.unwrap());
    assert!(ui.is_visible(block) && ui.is_visible(ind));
    let b = ui.displayed_box(block);
    assert!((b.l + b.r) / 2 - 400 <= 1);
    assert!(b.b <= 300);
    // The indicator is above the block, with the peace colour.
    let i = ui.displayed_box(ind);
    assert!(i.b < b.t);
    assert!(matches!(ui.base_image(ind), Some((ImageRef::File(p), _)) if p.contains("ffffe060")));
    c.set_in_combat(&mut ui, true);
    c.update(&mut ui, 0.1, &|e| {
        (e == 2).then(|| anchor(400.0, 300.0, 4.0))
    });
    assert!(matches!(ui.base_image(ind), Some((ImageRef::File(p), _)) if p.contains("ffff6040")));
    // Off screen: clamped and turned.
    let mut off = anchor(-500.0, 300.0, 4.0);
    off.on_screen = false;
    c.update(&mut ui, 0.2, &move |_| Some(off));
    let i = ui.displayed_box(ind);
    assert!(i.l >= 0);
    // Cleared.
    c.set_target(&mut ui, None, None);
    c.update(&mut ui, 0.3, &|_| None);
    assert!(!ui.is_visible(block) && !ui.is_visible(ind));
}

// ----- quickslots (§9, §17.3) -------------------------------------------------------------

#[test]
fn cooldown_frames_match_17_3() {
    let c = Cooldown::new(3.0, 3.0).unwrap();
    assert_eq!(c.frame(3.0), 59);
    assert_eq!(c.frame(4.5), 30);
    assert_eq!(c.frame(6.0), 0);
    assert!(c.done(6.0));
    assert!(Cooldown::new(0.0, 0.0).is_none());
    assert!(Cooldown::new(0.0, f64::NAN).is_none());
    // CUI5: never increases.
    let mut last = 59;
    for i in 0..=300 {
        let f = c.frame(3.0 + f64::from(i) * 0.01);
        assert!(f <= last && (0..=59).contains(&f));
        last = f;
    }
}

fn jab() -> SlotView {
    SlotView {
        kind: SlotKind::Skill(77),
        label: "Jab".into(),
        icon: None,
        tooltip: Some(0x7000_0077),
    }
}

#[test]
fn fallback_slots_cooldown_alert_playing_and_click() {
    let mut ui = bare_ui();
    let mut hud = look::Hud::default();
    let h = hud.root(&mut ui).unwrap();
    let mut s = SkillSlots::new(&cv());
    s.fallback_bar(&mut ui, h, &NoShortcuts);
    assert!(s.is_fallback());
    s.set_slot(&mut ui, 0, Some(jab()), 0.0);
    s.cooldown_started(&mut ui, 77, 3.0, 3.0);
    assert_eq!(s.frame(0), Some(59));
    s.update(&mut ui, 4.5);
    assert_eq!(s.frame(0), Some(30));
    // A second slot receiving the skill starts with the remaining time.
    s.set_slot(&mut ui, 1, Some(jab()), 4.5);
    assert_eq!(s.frame(1), Some(30));
    s.update(&mut ui, 6.0);
    assert_eq!(s.frame(0), None);
    assert_eq!(
        s.flags(0, 6.5),
        (false, false, true),
        "skillavailable after D ≥ 2"
    );
    assert_eq!(s.flags(0, 7.0), (false, false, false));
    // A short cooldown gives no alert.
    s.cooldown_started(&mut ui, 77, 1.0, 10.0);
    s.cooldown_ended(&mut ui, 77, 10.2);
    assert_eq!(s.flags(0, 10.3), (false, false, false));
    s.set_casting(&mut ui, Some(77), 11.0);
    assert!(s.flags(0, 11.0).0);
    s.set_casting(&mut ui, None, 11.5);
    assert!(!s.flags(0, 11.5).0);
    s.error(&mut ui, 0, 12.0);
    assert!(s.flags(0, 12.5).1);
    // Click: a filled slot is used; an empty one is not.
    let slot0 = tessera_ui::game::subtree(&ui, h)
        .into_iter()
        .find(|k| s.slot_at(&ui, *k) == Some(0))
        .unwrap();
    let ev = click(&ui, slot0, ui.vocab().msgs.mouse_click);
    assert_eq!(s.handle_event(&ui, &ev), Some(CombatAction::UseShortcut(0)));
    let slot5 = tessera_ui::game::subtree(&ui, h)
        .into_iter()
        .find(|k| {
            s.slot_at(&ui, *k) == Some(5) && ui.element_id(*k).is_some_and(|i| i >= 0x7EF0_0000)
        })
        .unwrap();
    let ev = click(&ui, slot5, ui.vocab().msgs.mouse_click);
    assert_eq!(s.handle_event(&ui, &ev), None);
    // Hostile indices are ignored.
    s.set_slot(&mut ui, -1, Some(jab()), 0.0);
    s.set_slot(&mut ui, 5000, Some(jab()), 0.0);
    assert!(s.slot(-1).is_none() && s.slot(5000).is_none());
}

// ----- procedural images ------------------------------------------------------------------

#[test]
fn procedural_images_parse_and_draw() {
    let ImageRef::File(p) = sweep_ref(0xA000_0000, 35, 59) else {
        panic!()
    };
    let full = procedural_image(&p).unwrap();
    assert_eq!((full.width(), full.height()), (35, 35));
    let ImageRef::File(p0) = sweep_ref(0xA000_0000, 35, 0) else {
        panic!()
    };
    let none = procedural_image(&p0).unwrap();
    let ImageRef::File(ph) = sweep_ref(0xA000_0000, 34, 30) else {
        panic!()
    };
    let half = procedural_image(&ph).unwrap();
    let count =
        |i: &tessera_material::ui_image::UiImage| i.pixels().iter().filter(|p| **p != 0).count();
    assert_eq!(count(&full), 35 * 35);
    assert_eq!(count(&none), 0);
    let n = count(&half);
    assert!((n as i32 - 34 * 34 / 2).abs() < 40, "{n}");
    // The half sweep covers the right half (clockwise from 12 o'clock).
    assert_ne!(half.pixels()[(10 * 34 + 30) as usize], 0);
    assert_eq!(half.pixels()[(10 * 34 + 3) as usize], 0);
    let ImageRef::File(t) = triangle_ref(0xFFFF_E060, 16, 12, 0) else {
        panic!()
    };
    let tri = procedural_image(&t).unwrap();
    // Pointing down: the top row is wide, the bottom row narrow.
    let row = |y: u32| {
        (0..16)
            .filter(|x| tri.pixels()[(y * 16 + x) as usize] != 0)
            .count()
    };
    assert!(row(0) > row(11));
    for bad in [
        "tessera:look/tri/zz/16x12/0",
        "tessera:look/tri/ffffffff/0x12/0",
        "tessera:look/tri/ffffffff/16x12/400",
        "tessera:look/sweep/ffffffff/9999/3",
        "tessera:look/sweep/ffffffff/35/60",
        "tessera:look/sweep/ffffffff/35/3/x",
        "tessera:look/other/ffffffff",
        "elsewhere",
    ] {
        assert!(procedural_image(bad).is_none(), "{bad}");
    }
}

#[test]
fn fallback_hud_creates_every_element() {
    let (ui, c) = fallback_ui();
    assert!(c.player.as_ref().is_some_and(UnitFrame::is_fallback));
    assert!(c.target.as_ref().is_some_and(UnitFrame::is_fallback));
    assert!(c.player_cast.as_ref().is_some_and(CastBar::is_fallback));
    assert!(c.target_cast.as_ref().is_some_and(CastBar::is_fallback));
    assert!(c.floaty.as_ref().is_some_and(FloatingTexts::is_fallback));
    assert!(c.marker.as_ref().is_some_and(SelectionMarker::is_fallback));
    assert!(
        c.combat_state
            .as_ref()
            .is_some_and(CombatStateDisplay::is_fallback)
    );
    assert!(c.defeat.as_ref().is_some_and(DefeatPanel::is_fallback));
    // Fallback text elements carry the look's pixel size.
    let px = c.vocab.text.text_px;
    let n = tessera_ui::game::subtree(&ui, c.hud.existing().unwrap())
        .into_iter()
        .filter(|k| ui.class(*k) == Some(WidgetClass::Text) && ui.prop(*k, px).int() == 14)
        .count();
    assert!(n >= 3, "{n}");
}

// ----- live timer rows of skill and effect tooltips (skill-tooltips.md §10.2, §12.2) ------

#[test]
fn live_timer_rows_count_down_and_hide_with_their_effect() {
    use tessera_ui::game::CharVocab;
    use tessera_ui::game::examine::{ExamRow, ExamTemplate, ExamTooltip, ExamTooltips, LiveTimers};
    const EX: u32 = 0x22FF_7100;
    let ch = CharVocab::default();
    let vv = v();
    let tmpl = |id: u32| {
        strukt(vec![
            (vv.pids.lb_template_layout, did(EX)),
            (vv.pids.lb_template_element, en(id)),
        ])
    };
    let mut list = typed(0x1000_0002, ch.types.exam_list_box, 5, 5, 229, 90);
    list = prop(
        list,
        vv.pids.lb_templates,
        array(
            vv.pids.lb_templates,
            vec![tmpl(0x1000_0010), tmpl(0x1000_0011), tmpl(0x1000_0012)],
        ),
    );
    list = prop(list, vv.pids.max_width, int(229));
    let frame = child(field(0x1000_0001, 0, 0, 239, 100), list);
    let name = typed(0x1000_0010, ch.types.exam_name, 0, 0, 180, 18);
    let desc = typed(0x1000_0011, vv.types.text, 0, 0, 220, 18);
    let time = typed(0x1000_0012, vv.types.text, 0, 0, 220, 18);
    let mut ui = ui_with(
        env_with(vec![layout(EX, 1024, 768, vec![frame, name, desc, time])]),
        1024,
        768,
    );
    ch.register_classes(&mut ui);
    let input = ExamTooltip {
        frame: ExamTemplate {
            layout: EX,
            element: 0x1000_0001,
        },
        rows: vec![
            ExamRow::text("Bleeding"),
            ExamRow::empty(),
            ExamRow::text("4s"),
        ],
        ..ExamTooltip::default()
    };
    let t = ExamTooltips::new(ch);
    let shown = t.show(&mut ui, &input, (100, 100)).unwrap();
    assert_eq!(shown.hover.by_template.len(), 3);
    assert!(
        shown.hover.by_template[1].is_none(),
        "the empty row was removed"
    );
    let live = LiveTimers::new(&ui, &shown.hover, &[(2, 0), (1, 1)], Some(42));
    assert_eq!(live.len(), 1);
    let mut src = |_timer: usize, now: f64| Some(format!("{}s", (4.0 - now).ceil()));
    assert!(live.update(&mut ui, 1.5, &mut src));
    let row = shown.hover.by_template[2].unwrap();
    assert_eq!(ui.text_of(row), "3s");
    assert!(!live.hide_if(&mut ui, 7));
    assert!(live.hide_if(&mut ui, 42));
    assert_eq!(ui.tooltip_panel(), None);
}

// ----- install skill displays over a quickslot bar (§9.3, §9.4, TR33) ---------------------

struct SkillAt3;

impl tessera_ui::game::Shortcuts for SkillAt3 {
    fn shortcut(&self, index: i32) -> tessera_ui::game::Shortcut {
        if index == 3 {
            tessera_ui::game::Shortcut::Kind(9)
        } else {
            tessera_ui::game::Shortcut::Empty
        }
    }

    fn content_template(&self, kind: u32) -> Option<u32> {
        (kind == 9).then_some(cv().ids.skill_slot)
    }
}

/// A quickslot bar over the install-look skill display, with a skill kind at index 3.
fn install_skill_bar() -> (Ui, tessera_ui::game::GamePanels) {
    use tessera_ui::game::{GameLayouts, GamePanels, GameVocab, SandboxData};
    const QS: u32 = 0x22FF_7200;
    let g = GameVocab::default();
    let c = cv();
    let (vv, pp) = (v().types, v().pids);
    let list = prop(
        prop(
            typed(0x1000_0501, vv.list_box, 0, 0, 420, 35),
            pp.lb_horizontal,
            b(true),
        ),
        pp.lb_max_columns,
        int(12),
    );
    let bar = child(
        prop(
            typed(0x1000_0500, g.types.quickslot_bar, 0, 0, 420, 35),
            g.pids.quickslot_listbox,
            en(0x1000_0501),
        ),
        list,
    );
    let button = typed(g.ids.quickslot_button, vv.button, 0, 0, 35, 35);
    let mut display = field(c.ids.skill_slot, 0, 0, 35, 35);
    display = child(
        display,
        typed(c.ids.skill_slot_icon, vv.button, 3, 3, 32, 32),
    );
    display = child(
        display,
        prop(
            prop(
                typed(c.ids.skill_slot_timer, vv.meter, 3, 3, 32, 32),
                pp.meter_frame,
                b(true),
            ),
            pp.meter_frame_array,
            array(
                pp.meter_frame_array,
                (0..60).map(|i| did(0x4100_7E35 + i)).collect(),
            ),
        ),
    );
    for (id, st) in [
        (c.ids.skill_slot_playing, c.states.playing),
        (c.ids.skill_slot_available, c.states.skill_available),
        (c.ids.skill_slot_error, c.states.skill_error),
    ] {
        display = child(
            display,
            state(field(id, 3, 3, 32, 32), st, 0, vec![], vec![]),
        );
    }
    let empty = field(g.ids.quickslot_empty, 0, 0, 35, 35);
    let mut ui = ui_one(QS, 420, 35, vec![bar, button, display, empty], 1024, 768);
    let k = ui.open_panel(Did(QS), 0x1000_0500).unwrap();
    let mut gp = GamePanels::new(
        g.clone(),
        GameLayouts {
            quickslots: Some(Did(QS)),
            ..GameLayouts::default()
        },
    );
    gp.attach(&mut ui, k, &SandboxData::default(), &SkillAt3);
    (ui, gp)
}

#[test]
fn install_skill_slot_runs_the_meter_and_reports_clicks() {
    let g = tessera_ui::game::GameVocab::default();
    let c = cv();
    let (mut ui, gp) = install_skill_bar();
    let mut s = SkillSlots::new(&c);
    s.set_slot(&mut ui, 3, Some(jab()), 0.0);
    s.attach_bars(&mut ui, &gp.bars, g.ids.quickslot_button, 0.0);
    assert!(!s.is_fallback());
    let slot = gp.bars[0].slots[3].button;
    // A skill without an icon (a literal) shows the §13.7 content in the install slot
    // (TR32, TR41): a box with the label's first two characters over the icon.
    let icon = find_below(&ui, slot, c.ids.skill_slot_icon).unwrap();
    let boxes = ui.children(icon);
    assert_eq!(boxes.len(), 1, "one fallback box over the icon");
    assert!(ui.is_visible(boxes[0]));
    let texts: Vec<String> = ui
        .children(boxes[0])
        .into_iter()
        .map(|k| ui.text_of(k))
        .collect();
    assert!(texts.iter().any(|t| t == "Ja"), "{texts:?}");
    s.set_slot(&mut ui, 3, None, 0.0);
    assert!(ui.children(icon).is_empty(), "removed with the content");
    s.set_slot(&mut ui, 3, Some(jab()), 0.0);
    let timer = find_below(&ui, slot, c.ids.skill_slot_timer).unwrap();
    assert_eq!(
        ui.prop(slot, c.pids.tooltip_did_override).value(),
        Some(&did(0x7000_0077))
    );
    assert!(ui.examines(slot));
    ui.set_time(3.0);
    s.cooldown_started(&mut ui, 77, 3.0, 3.0);
    assert!(ui.is_visible(timer));
    let p = ui.vocab().pids;
    assert!((ui.prop(timer, p.meter_position).float() - 1.0).abs() < EPS);
    assert!(ui.prop(timer, p.meter_smooth).bool());
    assert!((ui.prop(timer, p.meter_smooth_duration).float() - 3.0).abs() < EPS);
    s.update(&mut ui, 6.0);
    assert!(!ui.is_visible(timer));
    let avail = find_below(&ui, slot, c.ids.skill_slot_available).unwrap();
    assert!(ui.is_visible(avail));
    assert_eq!(ui.state(avail), Some(c.states.skill_available));
    s.update(&mut ui, 7.1);
    assert!(!ui.is_visible(avail));
    s.set_casting(&mut ui, Some(77), 8.0);
    let playing = find_below(&ui, slot, c.ids.skill_slot_playing).unwrap();
    assert!(ui.is_visible(playing));
    let ev = click(&ui, slot, ui.vocab().msgs.button_click);
    assert_eq!(s.handle_event(&ui, &ev), Some(CombatAction::UseShortcut(3)));
    assert_eq!(s.slot_at(&ui, timer), Some(3));
    s.detach(&mut ui);
    assert_eq!(s.handle_event(&ui, &ev), None);
}

// ----- review fixes ------------------------------------------------------------------------

#[test]
fn install_slot_filled_during_a_cooldown_and_a_cast_runs_the_meter_and_plays() {
    let g = tessera_ui::game::GameVocab::default();
    let c = cv();
    let (mut ui, gp) = install_skill_bar();
    let mut s = SkillSlots::new(&c);
    let other = SlotView {
        kind: SlotKind::Skill(78),
        ..jab()
    };
    s.set_slot(&mut ui, 3, Some(other), 0.0);
    s.attach_bars(&mut ui, &gp.bars, g.ids.quickslot_button, 0.0);
    let slot = gp.bars[0].slots[3].button;
    let timer = find_below(&ui, slot, c.ids.skill_slot_timer).unwrap();
    let playing = find_below(&ui, slot, c.ids.skill_slot_playing).unwrap();
    let p = ui.vocab().pids;
    s.cooldown_started(&mut ui, 77, 10.0, 1.0);
    s.set_casting(&mut ui, Some(77), 1.0);
    assert!(!ui.is_visible(timer));
    // TR35: the skill arrives with 9.5 s of its 10 s cooldown left.
    s.set_slot(&mut ui, 3, Some(jab()), 1.5);
    s.update(&mut ui, 1.6);
    assert!(s.frame(3).is_some());
    assert!(ui.is_visible(timer));
    assert!((ui.prop(timer, p.meter_position).float() - 0.95).abs() < EPS);
    assert!(ui.prop(timer, p.meter_smooth).bool());
    assert!((ui.prop(timer, p.meter_smooth_duration).float() - 9.5).abs() < EPS);
    assert!(ui.prop(timer, p.meter_goal).float().abs() < EPS);
    // TR36: the skill being cast shows `playing` in its new slot.
    assert!(s.flags(3, 1.6).0);
    assert!(ui.is_visible(playing));
    // §11: a bar rebuild reapplies the running cooldown to the slot elements.
    ui.set_property(timer, p.meter_position, PropertyValue::Float(0.0));
    ui.set_shown(timer, false);
    s.attach_bars(&mut ui, &gp.bars, g.ids.quickslot_button, 6.0);
    assert!(ui.is_visible(timer));
    assert!((ui.prop(timer, p.meter_position).float() - 0.5).abs() < EPS);
    assert!((ui.prop(timer, p.meter_smooth_duration).float() - 5.0).abs() < EPS);
    s.set_casting(&mut ui, None, 6.0);
    s.set_slot(&mut ui, 3, Some(jab()), 6.1);
    assert!(!s.flags(3, 6.1).0, "the same skill keeps its state");
}

#[test]
fn remembered_cooldowns_are_capped() {
    use tessera_ui::game::quickslot::MAX_COOLDOWNS;
    let mut ui = bare_ui();
    let mut s = SkillSlots::new(&cv());
    for k in 0..(MAX_COOLDOWNS as u64 + 500) {
        s.cooldown_started(&mut ui, k, 100.0 + k as f64, 0.0);
    }
    assert_eq!(s.remembered_cooldowns(), MAX_COOLDOWNS);
    // The one ending first was dropped; a slot receiving the newest one still starts it.
    s.set_slot(
        &mut ui,
        0,
        Some(SlotView {
            kind: SlotKind::Skill(MAX_COOLDOWNS as u64 + 499),
            ..jab()
        }),
        1.0,
    );
    assert!(s.cooldown(0).is_some());
    s.set_slot(
        &mut ui,
        1,
        Some(SlotView {
            kind: SlotKind::Skill(0),
            ..jab()
        }),
        1.0,
    );
    assert!(s.cooldown(1).is_none());
}

const F: u32 = 0x22FF_7300;

/// A synthetic `floatytext` layout: the name block with the mini vitals in the install
/// order (§3.5 OPEN), the indicator and the damage text.
fn floaty_layout() -> tessera_formats::ui_layout::UiLayout {
    let i = cv().ids;
    let s = cv().states;
    let vv = v();
    let meter = |id: u32| typed(id, vv.types.meter, 0, 0, 100, 9);
    let coloured = |d| {
        let d = state(d, s.meter_green, 0, vec![], vec![]);
        let d = state(d, s.meter_yellow, 0, vec![], vec![]);
        state(d, s.meter_grey, 0, vec![], vec![])
    };
    let health = child(
        child(
            field(i.opponent_health_meter.wrapping_add(0x100), 2, 2, 100, 9),
            coloured(meter(i.opponent_health_meter)),
        ),
        meter(i.opponent_temp_health),
    );
    let rage = child(field(i.rage_field, 2, 15, 100, 9), meter(i.rage_meter));
    let power = child(
        child(
            field(i.opponent_power_field, 2, 15, 100, 9),
            meter(i.opponent_power_meter),
        ),
        meter(i.opponent_temp_power),
    );
    let vitals = child(
        child(
            child(field(i.entity_mini_vitals, 0, 160, 105, 27), health),
            rage,
        ),
        power,
    );
    let induction = child(
        field(i.entity_mini_induction, 0, 187, 104, 13),
        coloured(meter(0x1000_7777)),
    );
    let mut block = field(i.selection_name_block, 0, 0, 512, 184);
    block = child(block, typed(i.entity_name, vv.types.text, 0, 0, 512, 80));
    block = child(block, vitals);
    block = child(block, induction);
    let ind = field(i.selection_indicator, 0, 0, 125, 156);
    let dmg = typed(i.damage_text, vv.types.text, 0, 0, 256, 40);
    layout(F, 1024, 768, vec![block, ind, dmg])
}

#[test]
fn install_mini_vitals_use_their_meters_and_hostile_settings_do_not_panic() {
    let mut ui = ui_with(env_with(vec![floaty_layout()]), 1024, 768);
    let c = cv();
    let hostile = FloatySettings {
        near_range: f32::NAN,
        far_range: f32::INFINITY,
        far_scale: f32::NEG_INFINITY,
        crit_v_offset: f32::NAN,
        ..FloatySettings::default()
    };
    let mut combat = CombatUi::new(
        &mut ui,
        c,
        CombatLayouts {
            floatytext: Some(Did(F)),
            ..CombatLayouts::default()
        },
        CombatRoots::default(),
        hostile,
    );
    assert!(!combat.marker.as_ref().unwrap().is_fallback());
    assert_eq!(
        combat.floaty.as_ref().unwrap().settings(),
        &FloatySettings::default()
    );
    let boar = FrameView {
        entity: 2,
        name: "Grey boar".into(),
        has_vitals: true,
        ..FrameView::default()
    };
    let view = SelectionView {
        name: "Grey boar".into(),
        health: Some(Vital::new(3.0, 10.0)),
        power: Some(Vital::new(7.0, 10.0)),
        health_colour: HealthColour::Green,
        ..SelectionView::default()
    };
    combat.set_target(&mut ui, Some(&boar), Some(view));
    let mut c2 = cast(4.0);
    c2.interruptible = true;
    combat.cast_started(&mut ui, CastOwner::Target, &c2, 0.0);
    let mut off = anchor(5000.0, 300.0, 10.0);
    off.on_screen = false;
    combat.update(&mut ui, 1.0, &move |_| Some(off));
    combat.floating_text(2, FloatyText::new("-5", 0xFFFF_FFFF, true));
    combat.update(&mut ui, 1.1, &move |_| Some(off));
    let block = combat.marker.as_ref().unwrap().elements().0.unwrap();
    let mp = ui.vocab().pids.meter_position;
    let at = |ui: &Ui, id: u32| ui.prop(find_below(ui, block, id).unwrap(), mp).float();
    assert!((at(&ui, c.ids.opponent_health_meter) - 0.3).abs() < EPS);
    assert!((at(&ui, c.ids.opponent_power_meter) - 0.7).abs() < EPS);
    assert_eq!(at(&ui, c.ids.opponent_temp_health), 0.0);
    assert_eq!(at(&ui, c.ids.opponent_temp_power), 0.0);
    let hm = find_below(&ui, block, c.ids.opponent_health_meter).unwrap();
    assert_eq!(ui.state(hm), Some(c.states.meter_green));
    let im = find_below(&ui, block, 0x1000_7777).unwrap();
    assert_eq!(
        ui.state(im),
        Some(c.states.meter_yellow),
        "§7.2 interruptible"
    );
    assert!((ui.prop(im, mp).float() - 0.275).abs() < EPS);
    // Non-finite placement inputs.
    let (bx, _) = selection::clamp_to_screen((f32::NAN, f32::NAN, f32::NAN, f32::NAN), (800, 600));
    assert!(bx.0.is_finite() && bx.1.is_finite());
    assert!(distance_scale(1.0, f32::NAN, 30.0, 1.2, 0.5).is_finite());
    assert!(distance_scale(1.0, 4.0, f32::INFINITY, f32::NAN, f32::NAN).is_finite());
}

/// Synthetic E7 and E11/E12 trees in the install order (§5.7 OPEN, §7.1).
fn bars_layout() -> tessera_formats::ui_layout::UiLayout {
    let i = cv().ids;
    let vv = v();
    let meter = |id: u32| typed(id, vv.types.meter, 0, 0, 92, 6);
    let mut vitals = field(i.selection_target_vitals, 44, 22, 94, 15);
    vitals = child(vitals, meter(0x1000_7778));
    vitals = child(
        vitals,
        child(
            child(meter(0x1000_7779), meter(i.selection_target_health)),
            meter(i.selection_target_temp_health),
        ),
    );
    vitals = child(vitals, meter(i.selection_target_dread));
    let mut e7 = field(i.selection_target, 200, 450, 140, 71);
    e7 = child(
        e7,
        typed(i.selection_target_name, vv.types.text, 39, 0, 100, 20),
    );
    e7 = child(e7, vitals);
    e7 = child(e7, field(i.selection_target_nonvital, 1, 6, 140, 48));
    let bar = |id: u32| {
        child(
            child(
                field(id, 32, 28, 255, 26),
                typed(i.progress_meter, vv.types.meter, 12, 6, 234, 13),
            ),
            typed(i.progress_label, vv.types.text, 11, 4, 238, 16),
        )
    };
    let mut drag = field(i.progress_dragbox, 380, 598, 320, 66);
    drag = child(drag, bar(i.progress_bar));
    drag = child(drag, bar(i.progress_bar_mounted));
    drag = child(drag, bar(i.progress_bar_music));
    let opp = child(
        field(i.opponent_progress_dragbox, 0, 500, 260, 66),
        bar(i.opponent_progress_bar),
    );
    layout(L + 1, 1024, 768, vec![e7, drag, opp])
}

#[test]
fn install_selection_target_and_cast_bar_variants() {
    let mut ui = ui_with(env_with(vec![bars_layout()]), 1024, 768);
    let c = cv();
    let e7 = ui.open_panel(Did(L + 1), c.ids.selection_target).unwrap();
    let drag = ui.open_panel(Did(L + 1), c.ids.progress_dragbox).unwrap();
    let odrag = ui
        .open_panel(Did(L + 1), c.ids.opponent_progress_dragbox)
        .unwrap();
    let mut combat = CombatUi::new(
        &mut ui,
        c,
        CombatLayouts::default(),
        CombatRoots {
            selection_target: Some(e7),
            player_progress: Some(drag),
            opponent_progress: Some(odrag),
            ..CombatRoots::default()
        },
        FloatySettings::default(),
    );
    assert!(!combat.selection_target.as_ref().unwrap().is_fallback());
    let boar = FrameView {
        entity: 2,
        name: "Grey boar".into(),
        has_vitals: true,
        ..FrameView::default()
    };
    combat.set_target(&mut ui, Some(&boar), None);
    let wolf = FrameView {
        entity: 3,
        name: "Wolf".into(),
        has_vitals: true,
        health: Some(Vital::new(5.0, 10.0)),
        ..FrameView::default()
    };
    combat.set_selection_target(&mut ui, Some(&wolf));
    let mp = ui.vocab().pids.meter_position;
    let m = |ui: &Ui, id: u32| find_below(ui, e7, id).unwrap();
    assert!((ui.prop(m(&ui, c.ids.selection_target_health), mp).float() - 0.5).abs() < EPS);
    assert!(
        !ui.prop(m(&ui, 0x1000_7778), mp).is_set(),
        "the background meter is left alone"
    );
    assert_eq!(
        ui.prop(m(&ui, c.ids.selection_target_dread), mp).float(),
        0.0
    );
    assert!(
        ui.prop(m(&ui, c.ids.selection_target_temp_health), mp)
            .is_set()
    );
    // TR27: the mounted and music variants are hidden; only the plain bar shows a cast.
    combat.cast_started(&mut ui, CastOwner::Player, &cast(2.0), 0.0);
    ui.frame_work();
    let k = |id: u32| find_below(&ui, drag, id).unwrap();
    assert!(ui.is_visible(k(c.ids.progress_bar)));
    assert!(!ui.is_visible(k(c.ids.progress_bar_mounted)));
    assert!(!ui.is_visible(k(c.ids.progress_bar_music)));
    // The player bar's click registration does not make the opponent bar's shared parts
    // mouse-visible.
    let om = find_below(&ui, odrag, c.ids.progress_meter).unwrap();
    let ol = find_below(&ui, odrag, c.ids.progress_label).unwrap();
    assert!(!ui.mouse_visible(om) && !ui.mouse_visible(ol));
    let pbar = k(c.ids.progress_bar);
    assert!(ui.mouse_visible(pbar));
    let ev = click(&ui, pbar, ui.vocab().msgs.mouse_click);
    assert_eq!(
        combat.handle_event(&mut ui, &ev),
        vec![CombatAction::CancelCast]
    );
    // A new selection hides E7 until the client refills it (CUI1).
    let post = FrameView {
        entity: 4,
        name: "Signpost".into(),
        ..FrameView::default()
    };
    combat.set_target(&mut ui, Some(&post), None);
    assert!(!ui.is_visible(e7));
    // Teardown leaves no registration behind.
    combat.detach(&mut ui);
    assert!(combat.player_cast.is_none());
    assert_eq!(combat.handle_event(&mut ui, &ev), vec![]);
}

#[test]
fn fallback_selection_target_without_vitals_shows_its_name() {
    let (mut ui, mut c) = fallback_ui();
    let boar = FrameView {
        entity: 2,
        name: "Grey boar".into(),
        has_vitals: true,
        ..FrameView::default()
    };
    c.set_target(&mut ui, Some(&boar), None);
    let post = FrameView {
        entity: 3,
        name: "Signpost".into(),
        has_vitals: false,
        name_colour: Some(NameColour::Object),
        ..FrameView::default()
    };
    c.set_selection_target(&mut ui, Some(&post));
    let root = c.selection_target.as_ref().unwrap().root();
    assert!(ui.is_visible(root));
    let texts: Vec<ElementKey> = tessera_ui::game::subtree(&ui, root)
        .into_iter()
        .filter(|k| ui.class(*k) == Some(WidgetClass::Text) && ui.is_displayed(*k))
        .collect();
    let named = texts
        .into_iter()
        .filter(|k| ui.text_of(*k) == "Signpost")
        .count();
    assert_eq!(named, 1);
}

#[test]
fn player_defeat_hides_the_cast_bar() {
    let (mut ui, mut c) = fallback_ui();
    c.cast_started(&mut ui, CastOwner::Player, &cast(3.0), 0.0);
    c.update(&mut ui, 0.5, &|_| None);
    let root = c.player_cast.as_ref().unwrap().root();
    assert!(ui.is_visible(root));
    assert!(c.player_defeated(&mut ui));
    assert!(c.player_cast.as_ref().unwrap().cast().is_none());
    ui.set_time(5.0);
    ui.frame_work();
    assert!(!ui.is_visible(root));
    assert_eq!(c.escape_action(), None);
}

#[test]
fn detach_removes_the_layers_and_the_fallback_root() {
    let mut ui = bare_ui();
    let screen = ui.screen_root();
    let before = ui.children(screen).len();
    let mut c = CombatUi::new(
        &mut ui,
        cv(),
        CombatLayouts::default(),
        CombatRoots::default(),
        FloatySettings::default(),
    );
    c.cast_started(&mut ui, CastOwner::Player, &cast(3.0), 0.0);
    c.player_defeated(&mut ui);
    let bar = c.player_cast.as_ref().unwrap().root();
    let ev = click(&ui, bar, ui.vocab().msgs.mouse_click);
    assert!(ui.children(screen).len() > before);
    c.detach(&mut ui);
    assert_eq!(ui.children(screen).len(), before);
    assert!(!ui.exists(bar));
    assert!(c.floaty.is_none() && c.marker.is_none() && c.defeat.is_none());
    assert_eq!(c.handle_event(&mut ui, &ev), vec![]);
}

#[test]
fn same_effect_id_keeps_cui7() {
    let mut r = EffectRows::default();
    let mut u = eff(1, 0.0, None);
    u.effect_id = 5;
    r.insert(u);
    let mut a = eff(2, 1.0, Some(10.0));
    a.effect_id = 9;
    r.insert(a);
    let mut b = eff(3, 0.5, Some(10.0));
    b.effect_id = 9;
    r.insert(b);
    assert!(r.satisfies_cui7());
    assert_eq!(
        r.row(0).iter().map(|e| e.instance).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    let mut c2 = eff(4, 2.0, Some(10.0));
    c2.effect_id = 9;
    r.insert(c2);
    assert_eq!(
        r.row(0).iter().map(|e| e.instance).collect::<Vec<_>>(),
        vec![1, 4, 2, 3]
    );
    assert!(r.satisfies_cui7());
}

#[test]
fn install_effect_rows_close_up_and_refresh_reapplied_icons() {
    let mut ui = ui_with(env_with(vec![avatar_layout()]), 1024, 768);
    let c = cv();
    let opp = ui.open_panel(Did(L), c.ids.opponent_panel).unwrap();
    let mut combat = CombatUi::new(
        &mut ui,
        c,
        CombatLayouts {
            effects: Some(Did(L)),
            ..CombatLayouts::default()
        },
        CombatRoots {
            opponent_panel: Some(opp),
            ..CombatRoots::default()
        },
        FloatySettings::default(),
    );
    let mut harm = eff(1, 0.0, Some(10.0));
    harm.harmful = true;
    combat.effect_applied(EffectOwner::Target, harm.clone());
    combat.update(&mut ui, 0.1, &|_| None);
    let bar = combat.target_effects.as_ref().unwrap();
    assert!(!bar.is_fallback());
    let rows = bar.row_elements();
    assert_eq!(
        ui.layout_box(rows[1]).unwrap().t,
        0,
        "an empty first row has no height"
    );
    combat.effect_applied(EffectOwner::Target, eff(2, 0.0, Some(10.0)));
    combat.update(&mut ui, 0.2, &|_| None);
    assert_eq!(ui.layout_box(rows[1]).unwrap().t, 20);
    // A second `EffectApplied` of instance 1 updates its icon.
    harm.start = 5.0;
    harm.duration = Some(30.0);
    combat.effect_applied(EffectOwner::Target, harm);
    combat.update(&mut ui, 5.0, &|_| None);
    let icon = combat.target_effects.as_ref().unwrap().icon(1).unwrap();
    assert!((ui.prop(icon, c.pids.effect_timeout).float() - 35.0).abs() < EPS);
    assert!((ui.prop(icon, c.pids.effect_duration).float() - 5.0).abs() < EPS);
}

#[test]
fn evicted_floating_texts_leave_the_screen() {
    use tessera_ui::game::combat::floaty::MAX_ENTITIES;
    let (mut ui, mut c) = fallback_ui();
    let all = |e: EntityId| Some(anchor(100.0 + (e % 500) as f32, 400.0, 4.0));
    for i in 0..5 {
        c.floating_text(0, FloatyText::new(format!("-{i}"), 0xFFFF_FFFF, false));
    }
    let mut now = 0.0;
    for _ in 0..60 {
        now += 1.0 / 60.0;
        c.update(&mut ui, now, &all);
    }
    for e in 1..MAX_ENTITIES as u64 {
        c.floating_text(e, FloatyText::new("-1", 0xFFFF_FFFF, false));
    }
    for _ in 0..3 {
        now += 1.0 / 60.0;
        c.update(&mut ui, now, &all);
    }
    assert!(c.floating_text(9999, FloatyText::new("-2", 0xFFFF_FFFF, false)));
    now += 1.0 / 60.0;
    c.update(&mut ui, now, &all);
    let f = c.floaty.as_ref().unwrap();
    let visible = ui
        .children(f.root())
        .into_iter()
        .filter(|k| ui.is_visible(*k))
        .count();
    assert_eq!(visible, f.shown().len());
}

#[test]
fn text_properties_and_pip_type_resolve_by_label() {
    let mut c = CombatVocab::default();
    c.resolve_all(
        &mut |n| (n == "PipDisplay").then_some(0x1000_0042),
        &mut |_| None,
        &mut |_| None,
        &mut |n| match n {
            "UICore_Text_font_color" => Some(0x1000_0001),
            "UICore_Text_one_line" => Some(0x1000_0006),
            _ => None,
        },
    );
    assert_eq!(c.text.font_color, 0x1000_0001);
    assert_eq!(c.text.one_line, 0x1000_0006);
    assert_eq!(c.text.text_px, CombatVocab::default().text.text_px);
    let mut ui = bare_ui();
    let combat = CombatUi::new(
        &mut ui,
        c,
        CombatLayouts::default(),
        CombatRoots::default(),
        FloatySettings::default(),
    );
    assert_eq!(combat.pip_display_type, 0x1000_0042);
}

// ----- portraits, voice icons and member effect lists (fix-ups) ----------------------------

/// P4 and P5 with their portraits (the P5 portrait with an own image and a child, as
/// `Target_Image`), and P4's voice, squelched, sending-data and assistant icons at the
/// start of the name (`social-panels.md` §4.3; none hidden by the layout, SPI9).
fn portrait_layout() -> tessera_formats::ui_layout::UiLayout {
    let i = cv().ids;
    let vv = v();
    let mut root = field(i.avatar_panel, 0, 0, 280, 300);
    root = child(root, typed(i.player_name, vv.types.text, 87, 3, 191, 36));
    root = child(root, field(i.player_portrait, 21, 21, 62, 62));
    for id in [
        i.player_voice,
        i.player_voice_squelched,
        i.player_sending,
        i.player_assistant,
    ] {
        root = child(root, field(id, 71, 11, 20, 20));
    }
    let mut opp = field(i.opponent_panel, 300, 0, 305, 300);
    let mut disp = field(i.opponent_display, 0, 20, 302, 300);
    disp = child(disp, typed(i.opponent_name, vv.types.text, 2, 3, 216, 36));
    let portrait = media(
        field(i.opponent_portrait, 220, 19, 62, 62),
        image_item(0x4100_0E4C, 2),
    );
    disp = child(disp, child(portrait, field(0x1000_0E01, 0, 0, 62, 62)));
    opp = child(opp, disp);
    layout(L, 1024, 768, vec![root, opp])
}

#[test]
fn install_frames_hide_voice_icons_and_portraits_without_a_scene() {
    let mut ui = ui_with(env_with(vec![portrait_layout()]), 1024, 768);
    let c = cv();
    let avatar = ui.open_panel(Did(L), c.ids.avatar_panel).unwrap();
    let opp = ui.open_panel(Did(L), c.ids.opponent_panel).unwrap();
    let f = |ui: &Ui, root, id: u32| find_below(ui, root, id).unwrap();
    let voice = [
        c.ids.player_voice,
        c.ids.player_voice_squelched,
        c.ids.player_sending,
        c.ids.player_assistant,
    ];
    assert!(voice.iter().all(|id| ui.is_visible(f(&ui, avatar, *id))));
    let mut combat = CombatUi::new(
        &mut ui,
        c,
        CombatLayouts::default(),
        CombatRoots {
            avatar_panel: Some(avatar),
            opponent_panel: Some(opp),
            ..CombatRoots::default()
        },
        FloatySettings::default(),
    );
    // `social-panels.md` TR14 on the player frame: no voice icon over the name.
    for id in voice {
        assert!(!ui.is_visible(f(&ui, avatar, id)), "{id:#x} hidden");
    }
    combat.set_player(
        &mut ui,
        &FrameView {
            entity: 1,
            name: "Tamsin".into(),
            has_vitals: true,
            ..FrameView::default()
        },
    );
    assert!(ui.is_visible(f(&ui, avatar, c.ids.player_name)));
    // `ui-widgets.md` §12: no scene, no portrait (not its own image, not its child).
    let t = FrameView {
        entity: 2,
        name: "Boar".into(),
        has_vitals: true,
        health: Some(Vital::new(3.0, 4.0)),
        ..FrameView::default()
    };
    combat.set_target(&mut ui, Some(&t), None);
    let portrait = f(&ui, opp, c.ids.opponent_portrait);
    let image = f(&ui, portrait, 0x1000_0E01);
    assert!(ui.is_visible(f(&ui, opp, c.ids.opponent_name)));
    assert!(!ui.is_visible(portrait));
    assert!(!ui.is_displayed(image), "nor its child");
    assert!(!ui.is_visible(f(&ui, avatar, c.ids.player_portrait)));
    let target = combat.target.as_ref().unwrap();
    assert_eq!(target.portrait(), Some(portrait));
    // A drawn scene shows the portrait again.
    target.set_portrait_scene(&mut ui, true);
    assert!(ui.is_displayed(portrait) && ui.is_displayed(image));
    target.set_portrait_scene(&mut ui, false);
    assert!(!ui.is_visible(portrait));
    // The fallback look has no portrait.
    let (_, fb) = fallback_ui();
    assert_eq!(fb.target.as_ref().unwrap().portrait(), None);
}

/// Two fellowship rows, each with a `FellowEffectDisplay`-like list.
const MEMBER_LIST: u32 = 0x1000_5E01;
const MEMBER_ROW: u32 = 0x1000_5E00;

fn member_rows_layout(columns: Option<i32>) -> tessera_formats::ui_layout::UiLayout {
    let mut panel = field(0x1000_5D00, 0, 0, 221, 452);
    for n in 0..2 {
        let mut list = field(MEMBER_LIST, 54, 37, 150, 37);
        if let Some(c) = columns {
            list = prop(list, cv().pids.effect_max_columns, int(c));
        }
        panel = child(panel, child(field(MEMBER_ROW, 0, n * 75, 221, 75), list));
    }
    let i = cv().ids;
    let icon = child(
        child(
            field(i.effect_icon, 0, 0, 36, 36),
            field(i.effect_border, 0, 0, 36, 36),
        ),
        field(i.effect_image, 2, 2, 32, 32),
    );
    layout(L, 1024, 768, vec![panel, icon])
}

/// An effect with a record icon (the install look of §8.1).
fn iconic(instance: u64, start: f64, duration: Option<f64>) -> EffectView {
    EffectView {
        icon: Some(tessera_ui::game::IconImage::Composed(ImageRef::Did(
            0x4100_0005,
        ))),
        ..eff(instance, start, duration)
    }
}

fn member_lists(ui: &Ui, panel: ElementKey) -> Vec<ElementKey> {
    tessera_ui::game::subtree(ui, panel)
        .into_iter()
        .filter(|k| ui.element_id(*k) == Some(MEMBER_LIST))
        .collect()
}

#[test]
fn member_effect_lists_follow_their_entity_tr16() {
    let mut ui = ui_with(env_with(vec![member_rows_layout(None)]), 1024, 768);
    let panel = ui.open_panel(Did(L), 0x1000_5D00).unwrap();
    let lists = member_lists(&ui, panel);
    assert_eq!(lists.len(), 2);
    let mut combat = CombatUi::new(
        &mut ui,
        cv(),
        CombatLayouts {
            effects: Some(Did(L)),
            ..CombatLayouts::default()
        },
        CombatRoots::default(),
        FloatySettings::default(),
    );
    // The kept effects of entity 7 (one timed, one untimed) bind with the list.
    let kept = |e: u64| -> Vec<EffectView> {
        if e == 7 {
            vec![iconic(1, 0.0, Some(30.0)), iconic(2, 1.0, None)]
        } else {
            Vec::new()
        }
    };
    combat.set_member_lists(&mut ui, &[(lists[0], Some(7)), (lists[1], None)], &kept);
    assert_eq!(
        combat.member_effects.len(),
        1,
        "a member without an entity: no list"
    );
    combat.update(&mut ui, 0.5, &|_| None);
    let bar = &combat.member_effects[0];
    assert_eq!(bar.owner(), EffectOwner::Member(7));
    assert_eq!(bar.display(), Some(lists[0]));
    assert_eq!(bar.max_columns(), MEMBER_MAX_COLUMNS);
    assert_eq!(bar.rows().row(0).len(), 2);
    // TR16 with §8.2: untimed first; 2 rows of at most 6 icons, the others kept.
    assert_eq!(bar.rows().row(0)[0].instance, 2);
    for n in 3..12 {
        combat.effect_applied(EffectOwner::Member(7), iconic(n, n as f64, Some(60.0)));
    }
    // Another entity's effect does not reach this row.
    combat.effect_applied(EffectOwner::Member(8), iconic(99, 0.0, Some(60.0)));
    combat.update(&mut ui, 12.0, &|_| None);
    let bar = &combat.member_effects[0];
    assert_eq!(bar.rows().len(), 11);
    assert!(bar.icon(99).is_none());
    let shown: Vec<u64> = bar.rows().shown(0, 6).iter().map(|e| e.instance).collect();
    assert_eq!(shown.len(), 6);
    for i in &shown {
        let icon = bar.icon(*i).unwrap();
        assert!(ui.is_visible(icon));
        assert_eq!(
            ui.layout_box(icon).unwrap().w(),
            20,
            "EffectDisplayList_EffectSize 20"
        );
        assert!(ui.is_ancestor_or_self(lists[0], icon));
    }
    let hidden = bar.rows().row(0)[6].instance;
    assert!(
        bar.icon(hidden).is_none(),
        "the seventh icon is kept, not shown"
    );
    // The examination hover finds a member icon (`skill-tooltips.md` §12.2).
    let first = bar.icon(shown[0]).unwrap();
    let image = find_below(&ui, first, cv().ids.effect_image).unwrap();
    assert_eq!(
        combat.effect_at(&ui, image),
        Some((EffectOwner::Member(7), shown[0]))
    );
    // Removal by the session.
    assert!(combat.effect_removed(EffectOwner::Member(7), shown[0]));
    assert!(!combat.effect_removed(EffectOwner::Member(7), shown[0]));
    // The row now shows another member: the old icons go, the new member's effects come.
    let other = |e: u64| -> Vec<EffectView> {
        if e == 8 {
            vec![iconic(40, 0.0, Some(5.0))]
        } else {
            Vec::new()
        }
    };
    combat.set_member_lists(&mut ui, &[(lists[0], Some(8))], &other);
    combat.update(&mut ui, 12.5, &|_| None);
    assert_eq!(combat.member_effects.len(), 1);
    let bar = &combat.member_effects[0];
    assert_eq!(bar.owner(), EffectOwner::Member(8));
    assert_eq!(bar.rows().len(), 1);
    let icons: Vec<ElementKey> = tessera_ui::game::subtree(&ui, lists[0])
        .into_iter()
        .filter(|k| ui.element_id(*k) == Some(cv().ids.effect_icon))
        .collect();
    assert_eq!(icons.len(), 1, "only the new member's icon is left");
    // Keeping the same binding keeps the bar (no rebuild).
    let before = combat.member_effects[0].row_elements();
    combat.set_member_lists(&mut ui, &[(lists[0], Some(8))], &other);
    assert_eq!(combat.member_effects[0].row_elements(), before);
    // No fellowship: every list goes, with its rows.
    combat.set_member_lists(&mut ui, &[], &other);
    assert!(combat.member_effects.is_empty());
    assert!(before.iter().all(|r| !ui.exists(*r)));
    // A destroyed row is dropped at the next binding; detach removes the rest.
    combat.set_member_lists(&mut ui, &[(lists[1], Some(8))], &other);
    ui.destroy(lists[1]);
    combat.update(&mut ui, 13.0, &|_| None);
    combat.set_member_lists(&mut ui, &[(lists[1], Some(8))], &other);
    assert!(combat.member_effects.is_empty());
    combat.set_member_lists(&mut ui, &[(lists[0], Some(8))], &other);
    combat.detach(&mut ui);
    assert!(combat.member_effects.is_empty());
}

#[test]
fn member_effect_lists_read_the_list_columns() {
    let mut ui = ui_with(env_with(vec![member_rows_layout(Some(3))]), 1024, 768);
    let panel = ui.open_panel(Did(L), 0x1000_5D00).unwrap();
    let lists = member_lists(&ui, panel);
    let mut combat = CombatUi::new(
        &mut ui,
        cv(),
        CombatLayouts::default(),
        CombatRoots::default(),
        FloatySettings::default(),
    );
    let kept = |_: u64| {
        (0..5)
            .map(|n| eff(n, n as f64, Some(9.0)))
            .collect::<Vec<_>>()
    };
    combat.set_member_lists(&mut ui, &[(lists[0], Some(3))], &kept);
    combat.update(&mut ui, 1.0, &|_| None);
    let bar = &combat.member_effects[0];
    assert_eq!(bar.max_columns(), 3);
    let shown = (0..5).filter(|n| bar.icon(*n).is_some()).count();
    assert_eq!(shown, 3);
    // Without the `effects` layout the icons take the §13.5 look (TR41).
    let icon = bar.icon(4).unwrap();
    assert_ne!(ui.element_id(icon), Some(cv().ids.effect_icon));
}
