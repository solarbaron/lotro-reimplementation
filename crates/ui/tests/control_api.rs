// SPDX-License-Identifier: GPL-3.0-or-later
//! The plugin-control element API (`lua-plugin-api.md` §9.6–§9.7, §13) on synthetic trees.
//! All values are invented.

mod ui_common;

use std::time::Duration;

use tessera_ui::control::plugin_colour;
use tessera_ui::geom::IBox;
use tessera_ui::{
    Background, ControlEvent, ControlKind, ElementKey, ImageRef, MemoryEnv, Ui, UiError, UiEvent,
};
use ui_common::*;

fn ui() -> Ui {
    ui_with(MemoryEnv::default(), 1920, 1080)
}

fn events_of(ui: &mut Ui, k: ElementKey) -> Vec<ControlEvent> {
    ui.take_events()
        .into_iter()
        .filter_map(|e| match e {
            UiEvent::Control { control, event } if control == k => Some(event),
            _ => None,
        })
        .collect()
}

fn window(ui: &mut Ui, w: i32, h: i32) -> ElementKey {
    let k = ui.create_control(ControlKind::Window, 1).unwrap();
    ui.control_set_size(k, w, h);
    ui.control_set_visible(k, true);
    k
}

#[test]
fn initial_geometry_and_tree() {
    let mut ui = ui();
    let w = ui.create_control(ControlKind::Window, 1).unwrap();
    let c = ui.create_control(ControlKind::Control, 1).unwrap();
    assert_eq!(ui.control_size(c), (50, 50));
    assert_eq!(ui.control_position(c), (0, 0));
    assert!(!ui.is_visible(c));
    assert_eq!(ui.control_parent(w), None);
    assert_eq!(ui.parent(w), Some(ui.screen_root()));
    ui.control_set_parent(c, Some(w)).unwrap();
    assert_eq!(ui.control_parent(c), Some(w));
    assert_eq!(ui.control_children(w), vec![c]);
    assert_eq!(ui.control_set_parent(c, Some(c)), Err(UiError::Cycle));
    assert_eq!(ui.control_set_parent(w, Some(c)), Err(UiError::Cycle));
    ui.control_set_parent(c, None).unwrap();
    assert_eq!(ui.control_parent(c), None);
    assert!(!ui.control_is_displayed(c));
}

#[test]
fn size_limits_worked_example() {
    let mut ui = ui();
    let c = ui.create_control(ControlKind::Control, 1).unwrap();
    ui.control_set_size(c, 100, 50);
    ui.control_set_limits(c, [None, None, Some(80), None]);
    assert_eq!(ui.control_size(c), (100, 50));
    let (w, _) = ui.control_size(c);
    ui.control_set_size(c, w, 60);
    assert_eq!(ui.control_size(c), (80, 60));
    ui.control_set_limits(c, [Some(90), None, Some(80), None]);
    let (_, h) = ui.control_size(c);
    ui.control_set_size(c, 10, h);
    assert_eq!(ui.control_size(c), (90, 60));
}

#[test]
fn edge_attachment_worked_examples() {
    type Case = ([i32; 4], (i32, i32, i32, i32));
    let cases: [Case; 6] = [
        ([0, 0, 0, 0], (10, 20, 59, 39)),
        ([0, 0, 5, 0], (10, 20, 159, 39)),
        ([4, 4, 5, 5], (110, 70, 159, 89)),
        ([0, 0, 5, 5], (10, 20, 159, 89)),
        ([1, 1, 1, 1], (125, 65, 174, 84)),
        ([3, 3, 3, 3], (15, 30, 88, 58)),
    ];
    for (att, want) in cases {
        let mut ui = ui();
        let w = window(&mut ui, 200, 100);
        let c = ui.create_control(ControlKind::Control, 1).unwrap();
        ui.control_set_parent(c, Some(w)).unwrap();
        ui.control_set_position(c, 10, 20);
        ui.control_set_size(c, 50, 20);
        ui.control_attach_edges(c, att);
        assert_eq!(ui.control_edges(c), att);
        ui.take_events();
        ui.control_set_size(w, 300, 150);
        let b = ui.layout_box(c).unwrap();
        assert_eq!((b.l, b.t, b.r, b.b), want, "{att:?}");
        // The parent's own resize lays children out with notifications.
        if want.2 - want.0 + 1 != 50 || want.3 - want.1 + 1 != 20 {
            assert!(
                events_of(&mut ui, c).contains(&ControlEvent::SizeChanged),
                "{att:?}"
            );
        }
    }
}

#[test]
fn colour_packing_is_exact_and_unclamped() {
    // Color(0.5, 0, 0, 1) → (127, 0, 0, 255).
    assert_eq!(plugin_colour(0.5, 0.0, 0.0, 1.0), 0x7F00_00FF);
    assert_eq!(plugin_colour(1.0, 0.999, 0.0, 0.0), 0xFFFE_0000);
    // 2.0 × 255 = 510 = 0x1FE: its high bit lands in the neighbouring channel.
    assert_eq!(plugin_colour(0.0, 0.0, 0.0, 2.0), 0x0000_01FE);
}

#[test]
fn backgrounds_blend_modes_and_back_colours() {
    let mut ui = ui();
    let c = ui.create_control(ControlKind::Control, 1).unwrap();
    assert_eq!(ui.control_blend_mode(c), 2);
    assert_eq!(
        ui.control_set_blend_mode(c, 9),
        Err(UiError::BadBlendMode(9))
    );
    ui.control_set_background(c, Background::Did(0x4100_0001));
    assert_eq!(ui.base_image(c), Some((ImageRef::Did(0x4100_0001), 2)));
    ui.control_set_blend_mode(c, 4).unwrap();
    assert_eq!(ui.base_image(c), Some((ImageRef::Did(0x4100_0001), 4)));
    ui.control_set_background(c, Background::File("A/b.tga".into()));
    assert_eq!(
        ui.base_image(c),
        Some((ImageRef::File("A/b.tga".into()), 4))
    );
    ui.control_set_background(c, Background::None);
    assert_eq!(ui.base_image(c), None);
    // SetBackColorBlendMode without a colour: a transparent-black Replace fill.
    ui.control_set_back_colour_blend_mode(c, 2).unwrap();
    assert_eq!(ui.overlay(c), Some((None, 0, 2)));
    ui.control_set_back_colour(c, Some([0.5, 0.0, 0.0, 1.0]));
    assert_eq!(ui.overlay(c), Some((None, 0x7F00_00FF, 2)));
    ui.control_set_back_colour(c, None);
    assert_eq!(ui.overlay(c), None);
    assert_eq!(ui.control_back_colour_blend_mode(c), 2);
}

#[test]
fn back_colour_paints_into_the_window_canvas() {
    let mut ui = ui();
    let w = window(&mut ui, 64, 64);
    let c = ui.create_control(ControlKind::Control, 1).unwrap();
    ui.control_set_parent(c, Some(w)).unwrap();
    ui.control_set_size(c, 10, 10);
    ui.control_set_visible(c, true);
    ui.control_set_back_colour(c, Some([0.5, 0.0, 0.0, 1.0]));
    let mut budget = tessera_ui::paint::Unlimited;
    for t in 0..3 {
        ui.paint(Duration::from_millis(16 * t), &mut budget);
    }
    assert_eq!(ui.surface_pixel(w, 5, 5), Some(0x7F00_00FF));
    assert_eq!(ui.surface_pixel(w, 20, 20), Some(0));
    let frame = ui.paint(Duration::from_millis(64), &mut budget);
    assert_eq!(frame.composition.entries.len(), 1);
    assert_eq!(
        frame.composition.entries[0].dest,
        IBox::from_xywh(0, 0, 64, 64).to_rect()
    );
}

#[test]
fn z_order_and_equal_z_insertion() {
    let mut ui = ui();
    let w = window(&mut ui, 100, 100);
    let a = ui.create_control(ControlKind::Control, 1).unwrap();
    let b2 = ui.create_control(ControlKind::Control, 1).unwrap();
    ui.control_set_parent(a, Some(w)).unwrap();
    ui.control_set_parent(b2, Some(w)).unwrap();
    // Equal ZOrder: the most recently added is on top (drawn later).
    assert_eq!(ui.children(w), vec![a, b2]);
    ui.control_set_z_order(a, 5);
    assert_eq!(ui.children(w), vec![b2, a]);
    assert_eq!(ui.control_z_order(a), 5);
    for k in [w, a, b2] {
        ui.control_set_visible(k, true);
    }
    ui.pointer_moved(10, 10);
    assert_eq!(ui.hovered(), Some(a));
    ui.control_set_mouse_visible(a, false);
    ui.pointer_moved(11, 10);
    assert_eq!(ui.hovered(), Some(b2));
}

#[test]
fn mouse_events_reach_the_source_control_only() {
    let mut ui = ui();
    let w = window(&mut ui, 100, 100);
    let c = ui.create_control(ControlKind::Button, 1).unwrap();
    ui.control_set_parent(c, Some(w)).unwrap();
    ui.control_set_visible(c, true);
    ui.take_events();
    ui.pointer_moved(10, 10);
    assert_eq!(
        events_of(&mut ui, c),
        vec![
            ControlEvent::MouseEnter,
            ControlEvent::MouseMove { x: 10, y: 10 }
        ]
    );
    ui.mouse_press(&cmd("SelectLeft"));
    ui.mouse_release(&cmd("SelectLeft"));
    let ev = events_of(&mut ui, c);
    assert_eq!(
        ev,
        vec![
            ControlEvent::MouseDown {
                button: 1,
                x: 10,
                y: 10
            },
            ControlEvent::MouseUp {
                button: 1,
                x: 10,
                y: 10
            },
            // The button's activation is queued while the release is delivered, so it comes
            // before the click message that follows the release (UB §4.6, §12.1).
            ControlEvent::Click,
            ControlEvent::MouseClick {
                button: 1,
                x: 10,
                y: 10
            },
        ]
    );
    // §9.7: a handler's calls queue their events after the current one, in call order.
    ui.control_set_size(c, 60, 20);
    ui.control_set_visible(c, false);
    assert_eq!(
        events_of(&mut ui, c),
        vec![ControlEvent::SizeChanged, ControlEvent::VisibleChanged]
    );
    ui.control_set_visible(c, true);
    ui.take_events();
    ui.pointer_moved(10, 11);
    assert!(ui.wheel(&cmd("ScrollUp")) || !ui.wheel(&cmd("ScrollUp")));
    assert!(events_of(&mut ui, c).contains(&ControlEvent::MouseWheel {
        direction: 1,
        x: 10,
        y: 11
    }));
    // Every hovered plugin control pushes ScrollableControls_Mouse (decision 0014 D5).
    assert!(
        ui.band()
            .iter()
            .any(|e| e.map == "ScrollableControls_Mouse" && e.owner == c)
    );
}

#[test]
fn double_click_and_mouse_hover() {
    let mut ui = ui();
    let w = window(&mut ui, 100, 100);
    ui.take_events();
    ui.pointer_moved(10, 10);
    ui.mouse_press(&cmd("SelectDblLeft"));
    ui.mouse_release(&cmd("SelectDblLeft"));
    assert!(
        events_of(&mut ui, w).contains(&ControlEvent::MouseDoubleClick {
            button: 1,
            x: 10,
            y: 10
        })
    );
    // MouseHover after more than 0.25 s, then every tick (§13.12.5).
    ui.set_time(0.25);
    ui.tick_hover();
    assert!(
        !events_of(&mut ui, w)
            .iter()
            .any(|e| matches!(e, ControlEvent::MouseHover { .. }))
    );
    ui.set_time(0.3);
    ui.tick_hover();
    ui.set_time(0.31);
    ui.tick_hover();
    let n = events_of(&mut ui, w)
        .iter()
        .filter(|e| matches!(e, ControlEvent::MouseHover { .. }))
        .count();
    assert_eq!(n, 2);
}

#[test]
fn focus_worked_example() {
    let mut ui = ui();
    let w1 = window(&mut ui, 100, 100);
    let w2 = window(&mut ui, 100, 100);
    let t1 = ui.create_control(ControlKind::TextBox, 1).unwrap();
    let t2 = ui.create_control(ControlKind::TextBox, 1).unwrap();
    ui.control_set_parent(t1, Some(w1)).unwrap();
    ui.control_set_parent(t2, Some(w2)).unwrap();
    ui.activate(w1);
    ui.control_focus(t1);
    assert!(ui.control_has_focus(t1));
    ui.take_events();
    ui.control_focus(t2);
    assert!(!ui.control_has_focus(t2));
    assert!(ui.take_events().is_empty());
    ui.activate(w2);
    let order: Vec<(ElementKey, ControlEvent)> = ui
        .take_events()
        .into_iter()
        .filter_map(|e| match e {
            UiEvent::Control { control, event } => Some((control, event)),
            _ => None,
        })
        .collect();
    assert_eq!(
        order,
        vec![
            (w1, ControlEvent::Deactivated),
            (t1, ControlEvent::FocusLost),
            (w2, ControlEvent::Activated),
            (t2, ControlEvent::FocusGained),
        ]
    );
}

#[test]
fn rotation_targets_and_memory() {
    let mut ui = ui();
    let w = ui.create_control(ControlKind::Window, 1).unwrap();
    // Hidden: no surface, mode becomes 1, nothing rotated.
    assert!(!ui.window_set_rotation(w, [0.0, 0.0, 30.0]));
    assert_eq!(ui.control_stretch_mode(w), 1);
    assert_eq!(ui.window_rotation(w), [0.0; 3]);
    ui.control_set_visible(w, true);
    assert!(ui.window_set_rotation(w, [0.0, 0.0, 90.0]));
    assert_eq!(ui.window_rotation(w), [0.0, 0.0, 90.0]);
    let mut budget = tessera_ui::paint::Unlimited;
    let fr = ui.paint(Duration::ZERO, &mut budget);
    let _ = fr;
    ui.control_set_visible(w, false);
    ui.control_set_visible(w, true);
    assert_eq!(ui.window_rotation(w), [0.0, 0.0, 90.0]);
}

#[test]
fn scale_and_apartments_and_subscriptions() {
    let mut ui = ui();
    let w = window(&mut ui, 300, 200);
    ui.control_set_position(w, 100, 100);
    assert_eq!(ui.window_set_scale(w, 0.0, false), Err(UiError::BadScale));
    ui.window_set_scale(w, 1.5, false).unwrap();
    assert!((ui.panel_scale(w) - 1.5).abs() < 1e-6);
    assert_eq!(ui.displayed_box(w).w(), 450);
    let a = ui.create_control(ControlKind::Control, 7).unwrap();
    let b2 = ui.create_control(ControlKind::Control, 7).unwrap();
    ui.control_set_wants_updates(b2, true);
    ui.control_set_wants_updates(a, true);
    assert_eq!(ui.update_subscribers(), vec![b2, a]);
    ui.control_set_wants_keys(a, true);
    assert_eq!(ui.key_subscribers(), vec![a]);
    ui.destroy_apartment(7);
    assert!(!ui.exists(a) && !ui.exists(b2));
    assert!(ui.update_subscribers().is_empty());
    assert!(ui.exists(w));
}

#[test]
fn opacity_stretch_mode_and_surfaces() {
    let mut ui = ui();
    let w = window(&mut ui, 64, 64);
    let c = ui.create_control(ControlKind::Control, 1).unwrap();
    ui.control_set_parent(c, Some(w)).unwrap();
    ui.control_set_visible(c, true);
    ui.control_set_opacity(c, 0.5);
    assert_eq!(ui.control_opacity(c), 0.5);
    assert!(!ui.owns_surface(c));
    ui.control_set_stretch_mode(c, 1);
    assert!(ui.owns_surface(c));
    ui.control_set_back_colour(c, Some([1.0, 1.0, 0.0, 0.0]));
    let mut budget = tessera_ui::paint::Unlimited;
    let mut fr = ui.paint(Duration::ZERO, &mut budget);
    for t in 1..4 {
        fr = ui.paint(Duration::from_millis(16 * t), &mut budget);
    }
    // The window, the nested control; the canvas above holds nothing.
    let opacities: Vec<f32> = fr.composition.entries.iter().map(|e| e.opacity).collect();
    assert!(opacities.contains(&0.5), "{opacities:?}");
    // A Stretch control resize changes its scale, not its layout box.
    ui.control_set_size(c, 100, 100);
    assert_eq!(ui.control_size(c), (50, 50));
    // Displayed width trunc((50 + 1) × 2 − 1) = 101 (UB §9.2.1).
    assert_eq!(ui.displayed_box(c).w(), 101);
}

/// `lua-plugin-api.md` §14.2.6 round-5 answer 1 (Tessera rule): a shown window's new element
/// gets the old surface's rotation at once, also when it starts without a surface of its own
/// (it gets the Stretch object mode, as `SetRotation` step 2 gave the old one).
#[test]
fn a_carried_rotation_rotates_a_shown_element_without_a_surface() {
    let mut ui = ui();
    let from = window(&mut ui, 200, 200);
    assert!(ui.window_set_rotation(from, [0.0, 0.0, 45.0]));
    assert!(ui.owns_surface(from));
    let to = ui.create_control(ControlKind::Control, 1).unwrap();
    ui.control_set_size(to, 200, 200);
    ui.control_set_visible(to, true);
    assert!(!ui.owns_surface(to), "starts without a surface");
    ui.control_carry_state(from, to);
    assert_eq!(ui.window_rotation(to), [0.0, 0.0, 45.0], "GetRotation");
    assert!(ui.owns_surface(to), "has a surface now");
    assert_eq!(
        tessera_ui::game::held_rotation(&ui, to),
        [0.0, 0.0, 45.0],
        "its surface holds the old surface's rotation"
    );
    assert_eq!(ui.control_stretch_mode(to), 1);
}
