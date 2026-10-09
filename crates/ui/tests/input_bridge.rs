// SPDX-License-Identifier: GPL-3.0-or-later
//! The UI side of the input bridge (decision 0014 D5, D7, D8) on synthetic trees.

mod ui_common;

use std::collections::BTreeMap;

use tessera_formats::did::Did;
use tessera_ui::{
    BandChange, Command, CommandOffer, ElementKey, Phase, Ui, UiChange, UiEvent, UiInput,
};
use ui_common::*;

const LAYOUT: u32 = 0x22FF_3000;

fn offer(ui: &mut Ui, phase: Phase, hold: u64, name: &str, id: Option<u32>) -> bool {
    ui.offer(&UiInput::Command(CommandOffer {
        phase,
        hold,
        command: Command {
            name: name.into(),
            id,
        },
        value: 1.0,
        position: None,
    }))
}

fn move_to(ui: &mut Ui, x: f64, y: f64) {
    ui.offer(&UiInput::PointerMoved { x, y });
}

fn panel_ui() -> (Ui, ElementKey, ElementKey) {
    let vv = v();
    let btn = z(typed(0x2, vv.types.button, 10, 10, 50, 20), 0, 1);
    let p = prop(
        child(field(0x1, 0, 0, 200, 200), btn),
        vv.pids.blocks_clicks,
        b(true),
    );
    let p = prop(p, vv.pids.activatable, b(true));
    let mut ui = ui_one(LAYOUT, 1024, 768, vec![p], 1024, 768);
    let k = ui.open_panel(Did(LAYOUT), 0x1).unwrap();
    let bk = ui.find_child(k, 0x2).unwrap();
    (ui, k, bk)
}

#[test]
fn mouse_button_verdicts() {
    let (mut ui, k, bk) = panel_ui();
    let vv = v();
    ui.register_handler(0x2, vv.msgs.mouse_click);
    move_to(&mut ui, 20.0, 15.0);
    assert!(offer(&mut ui, Phase::Start, 1, "SelectLeft", Some(0x56)));
    assert_eq!(ui.captured(), Some(bk));
    assert_eq!(ui.active_panel(), Some(k));
    ui.take_events();
    assert!(offer(&mut ui, Phase::End, 1, "SelectLeft", Some(0x56)));
    assert!(
        ui.take_events()
            .iter()
            .any(|e| matches!(e, UiEvent::Game { message } if message.msg == vv.msgs.mouse_click))
    );
    // On the panel background (blocks clicks only): consumed, not routed to game code.
    move_to(&mut ui, 150.0, 150.0);
    assert!(offer(&mut ui, Phase::Start, 2, "SelectLeft", Some(0x56)));
    offer(&mut ui, Phase::End, 2, "SelectLeft", Some(0x56));
    // Over the world: not consumed; the press deactivates the panel.
    move_to(&mut ui, 500.0, 500.0);
    assert!(!offer(&mut ui, Phase::Start, 3, "SelectLeft", Some(0x56)));
    assert_eq!(ui.active_panel(), None);
    assert!(!offer(&mut ui, Phase::End, 3, "SelectLeft", Some(0x56)));
}

#[test]
fn impulse_actions_and_ignored_action() {
    let (mut ui, _k, bk) = panel_ui();
    let mut table = BTreeMap::new();
    table.insert(0x32, 2u8);
    ui.set_toggle_types(table);
    move_to(&mut ui, 20.0, 15.0);
    assert!(offer(&mut ui, Phase::Start, 1, "TapLeft", Some(0x32)));
    assert_eq!(ui.captured(), None, "an Impulse makes no capture");
    assert!(!offer(&mut ui, Phase::End, 1, "TapLeft", Some(0x32)));
    // Action 0xAB is ignored entirely.
    ui.take_events();
    assert!(!offer(&mut ui, Phase::Start, 2, "Scroll", Some(0xAB)));
    assert!(ui.take_events().is_empty());
    let _ = bk;
}

#[test]
fn key_verdicts_and_broadcast() {
    let (mut ui, k, _bk) = panel_ui();
    // No keyboard target: not consumed, broadcast.
    assert!(!offer(&mut ui, Phase::Start, 1, "MoveForward", Some(0x01)));
    assert!(matches!(
        ui.take_events().last(),
        Some(UiEvent::KeyBroadcast { down: true, .. })
    ));
    ui.activate(k);
    // The active panel does not consume an unknown key.
    assert!(!offer(&mut ui, Phase::Start, 2, "MoveForward", Some(0x01)));
    assert!(!offer(&mut ui, Phase::Repeat, 2, "MoveForward", Some(0x01)));
    assert!(!offer(&mut ui, Phase::End, 2, "MoveForward", Some(0x01)));
    let ev = ui.take_events();
    let downs = ev
        .iter()
        .filter(|e| matches!(e, UiEvent::KeyBroadcast { down: true, .. }))
        .count();
    assert_eq!(downs, 2, "a Repeat is routed like a Start");
}

#[test]
fn wheel_band_pushes_follow_the_hover() {
    let vv = v();
    let pp = vv.pids;
    let bt = vv.types.button;
    let bar = child(
        prop(
            typed(0x11, vv.types.scrollbar, 300, 0, 10, 200),
            pp.sb_proportional,
            b(true),
        ),
        z(typed(vv.ids.widget, bt, 0, 0, 10, 10), 0, 1),
    );
    let list = prop(
        prop(
            typed(0x10, vv.types.list_box, 0, 0, 300, 200),
            pp.scrollable_vbar,
            en(0x11),
        ),
        pp.lb_max_columns,
        int(1),
    );
    let root = child(
        child(field(0x1, 0, 0, 600, 600), z(list, 0, 1)),
        z(bar, 0, 2),
    );
    let mut ui = ui_one(
        LAYOUT,
        1024,
        768,
        vec![root, field(0x20, 0, 0, 100, 50)],
        1024,
        768,
    );
    let r = ui.open_panel(Did(LAYOUT), 0x1).unwrap();
    let l = ui.find_child(r, 0x10).unwrap();
    for _ in 0..10 {
        let e = ui.create_element(Some(l), Did(LAYOUT), 0x20).unwrap();
        ui.list_add(l, e, None).unwrap();
    }
    ui.take_band_changes();
    move_to(&mut ui, 50.0, 50.0);
    let ch = ui.take_band_changes();
    assert!(
        ch.iter().any(|c| matches!(c, BandChange::Push { map, priority: 0, .. } if map == "ScrollableControls_Mouse")),
        "{ch:?}"
    );
    assert!(offer(&mut ui, Phase::Start, 1, "ScrollDown", Some(0x10)));
    assert_eq!(ui.scroll_state(l).unwrap().1, (0, 50));
    assert!(!offer(&mut ui, Phase::End, 1, "ScrollDown", Some(0x10)));
    // Leaving the list pops the entry.
    move_to(&mut ui, 500.0, 500.0);
    let ch = ui.take_band_changes();
    assert!(
        ch.iter()
            .any(|c| matches!(c, BandChange::Pop { map, .. } if map == "ScrollableControls_Mouse"))
    );
    assert!(ui.band().is_empty());
    // Over nothing the wheel is not consumed (the camera zooms).
    assert!(!offer(&mut ui, Phase::Start, 2, "ScrollDown", Some(0x10)));
}

#[test]
fn change_requests_apply_in_step_7_with_screen_coalesced() {
    let (mut ui, k, _bk) = panel_ui();
    ui.request(UiChange::Screen(800, 600));
    ui.request(UiChange::Move(k, 30, 40));
    ui.request(UiChange::Screen(1280, 720));
    ui.request(UiChange::Show(k, false));
    assert_eq!(ui.screen_size(), (1024, 768));
    ui.tick_hover();
    assert_eq!(ui.screen_size(), (1280, 720));
    assert!(!ui.is_visible(k));
    let resized = ui
        .take_events()
        .into_iter()
        .filter(|e| matches!(e, UiEvent::ScreenResized { .. }))
        .count();
    assert_eq!(resized, 1);
}

#[test]
fn step_7_hover_refresh_sees_geometry_changed_by_code() {
    let (mut ui, k, bk) = panel_ui();
    move_to(&mut ui, 20.0, 15.0);
    assert_eq!(ui.hovered(), Some(bk));
    // Code moves the button away under a resting pointer: the next refresh unhovers it.
    ui.move_to(bk, 100, 100);
    ui.tick_hover();
    assert_eq!(ui.hovered(), None);
    let _ = k;
}

#[test]
fn pointer_left_and_focus_lost() {
    let (mut ui, _k, bk) = panel_ui();
    move_to(&mut ui, 20.0, 15.0);
    assert_eq!(ui.hovered(), Some(bk));
    ui.offer(&UiInput::PointerLeft);
    assert_eq!(ui.hovered(), None);
    move_to(&mut ui, 20.0, 15.0);
    assert_eq!(ui.hovered(), Some(bk));
    ui.offer(&UiInput::FocusLost);
    assert_eq!(ui.hovered(), None);
}
