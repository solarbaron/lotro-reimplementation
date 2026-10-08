// SPDX-License-Identifier: GPL-3.0-or-later
//! The tree side of UI rendering (decision 0015 §1–§2, §5): surfaces, display lists,
//! damage and the composition list, on synthetic trees.

mod ui_common;

use std::time::Duration;

use tessera_formats::did::Did;
use tessera_ui::paint::{NodeContent, Unlimited};
use tessera_ui::{Ui, UiFrame};
use ui_common::*;

const LAYOUT: u32 = 0x22FF_4000;

fn frames(ui: &mut Ui, n: u64) -> UiFrame {
    let mut budget = Unlimited;
    let mut last = UiFrame::default();
    for t in 0..n {
        last = ui.paint(Duration::from_millis(16 * t + 1), &mut budget);
    }
    last
}

#[test]
fn root_panels_own_surfaces_and_compose_in_order() {
    let a = media(field(0x1, 10, 10, 32, 32), image_item(0x4100_0001, 2));
    let bb = media(
        z(field(0x2, 20, 20, 32, 32), -5, 0),
        image_item(0x4100_0002, 2),
    );
    let mut env = env_with(vec![layout(LAYOUT, 1024, 768, vec![a, bb])]);
    add_image(&mut env, 0x4100_0001, solid(32, 32, 0xFF11_2233));
    add_image(&mut env, 0x4100_0002, solid(32, 32, 0xFF44_5566));
    let mut ui = ui_with(env, 1024, 768);
    let ak = ui.open_panel(Did(LAYOUT), 0x1).unwrap();
    let bk = ui.open_panel(Did(LAYOUT), 0x2).unwrap();
    let fr = frames(&mut ui, 4);
    assert_eq!(fr.surfaces, 2);
    assert_eq!(fr.composition.entries.len(), 2);
    // Drawing order: a (z 0) behind b (z −5).
    assert_eq!(
        fr.composition.entries[0].dest,
        ui.displayed_box(ak).to_rect()
    );
    assert_eq!(
        fr.composition.entries[1].dest,
        ui.displayed_box(bk).to_rect()
    );
    assert_eq!(ui.surface_pixel(ak, 0, 0), Some(0xFF11_2233));
    // Hiding releases the surface and its entry.
    ui.set_shown(ak, false);
    let fr = frames(&mut ui, 2);
    assert_eq!(fr.composition.entries.len(), 1);
    assert!(ui.surface_of(ak).is_none());
}

#[test]
fn damage_repaints_a_changed_layer() {
    let vv = v();
    let e = state(
        media(field(0x1, 0, 0, 16, 16), image_item(0x4100_0001, 2)),
        0x50,
        0,
        vec![],
        vec![image_item(0x4100_0002, 2)],
    );
    let mut env = env_with(vec![layout(LAYOUT, 1024, 768, vec![e])]);
    add_image(&mut env, 0x4100_0001, solid(16, 16, 0xFFAA_0000));
    add_image(&mut env, 0x4100_0002, solid(16, 16, 0xFF00_AA00));
    let mut ui = ui_with(env, 1024, 768);
    let k = ui.open_panel(Did(LAYOUT), 0x1).unwrap();
    frames(&mut ui, 3);
    assert_eq!(ui.surface_pixel(k, 3, 3), Some(0xFFAA_0000));
    ui.switch_state(k, 0x50);
    frames(&mut ui, 3);
    assert_eq!(ui.surface_pixel(k, 3, 3), Some(0xFF00_AA00));
    let _ = vv;
}

#[test]
fn nested_stretch_surface_is_a_separate_entry_with_its_own_opacity() {
    let vv = v();
    let pp = vv.pids;
    let c = prop(
        prop(
            media(field(0x2, 4, 4, 8, 8), image_item(0x4100_0002, 2)),
            pp.object_mode,
            en(1),
        ),
        pp.object_opacity,
        f(0.25),
    );
    let late = z(
        media(field(0x3, 0, 0, 16, 16), image_item(0x4100_0003, 4)),
        -1,
        2,
    );
    let p = prop(
        media(field(0x1, 0, 0, 16, 16), image_item(0x4100_0001, 2)),
        pp.object_opacity,
        f(0.5),
    );
    let p = child(child(p, z(c, 0, 1)), late);
    let mut env = env_with(vec![layout(LAYOUT, 1024, 768, vec![p])]);
    add_image(&mut env, 0x4100_0001, solid(16, 16, 0xFF00_0001));
    add_image(&mut env, 0x4100_0002, solid(8, 8, 0xFF00_0002));
    add_image(&mut env, 0x4100_0003, solid(16, 16, 0x8000_0003));
    let mut ui = ui_with(env, 1024, 768);
    let k = ui.open_panel(Did(LAYOUT), 0x1).unwrap();
    let ck = ui.find_child(k, 0x2).unwrap();
    let fr = frames(&mut ui, 4);
    assert!(ui.surface_of(ck).is_some());
    let ops: Vec<f32> = fr.composition.entries.iter().map(|e| e.opacity).collect();
    // Root (0.5), nested child (0.25, its own only), the canvas above (root's 0.5).
    assert_eq!(ops, vec![0.5, 0.25, 0.5]);
    // The child is not painted into the root's canvas below.
    assert_eq!(ui.surface_pixel(k, 6, 6), Some(0xFF00_0001));
    assert_eq!(ui.surface_pixel(ck, 1, 1), Some(0xFF00_0002));
}

#[test]
fn mode_zero_root_panel_uses_a_stand_in_at_opacity_one() {
    let vv = v();
    let pp = vv.pids;
    let p = prop(
        prop(
            media(field(0x1, 0, 0, 8, 8), image_item(0x4100_0001, 2)),
            pp.object_mode,
            en(0),
        ),
        pp.object_opacity,
        f(0.3),
    );
    let mut env = env_with(vec![layout(LAYOUT, 1024, 768, vec![p])]);
    add_image(&mut env, 0x4100_0001, solid(8, 8, 0xFF12_3456));
    let mut ui = ui_with(env, 1024, 768);
    let k = ui.open_panel(Did(LAYOUT), 0x1).unwrap();
    assert!(!ui.owns_surface(k));
    let fr = frames(&mut ui, 3);
    assert_eq!(fr.composition.entries.len(), 1);
    assert_eq!(fr.composition.entries[0].opacity, 1.0);
}

#[test]
fn alpha_test_meter_threshold_is_a_composition_value() {
    let vv = v();
    let pp = vv.pids;
    let m = typed(0x2, vv.types.meter, 0, 0, 8, 8);
    let m = prop(
        prop(
            media(m, image_item(0x4100_0001, 2)),
            pp.meter_alphatest,
            b(true),
        ),
        pp.meter_position,
        f(0.25),
    );
    let p = child(field(0x1, 0, 0, 16, 16), z(m, 0, 1));
    let mut env = env_with(vec![layout(LAYOUT, 1024, 768, vec![p])]);
    add_image(&mut env, 0x4100_0001, solid(8, 8, 0xFF00_00FF));
    let mut ui = ui_with(env, 1024, 768);
    let k = ui.open_panel(Did(LAYOUT), 0x1).unwrap();
    let mk = ui.find_child(k, 0x2).unwrap();
    assert_eq!(ui.meter_alpha_threshold(mk), Some(0.75));
    let fr = frames(&mut ui, 3);
    assert!(
        fr.composition
            .entries
            .iter()
            .any(|e| (e.alpha_threshold - 0.75).abs() < 1e-6)
    );
}

#[test]
fn external_content_of_a_viewport_is_a_nested_entry() {
    let vv = v();
    let vp = typed(0x2, vv.types.viewport, 2, 2, 8, 8);
    let p = child(field(0x1, 0, 0, 16, 16), z(vp, 0, 1));
    let mut ui = ui_with(
        env_with(vec![layout(LAYOUT, 1024, 768, vec![p])]),
        1024,
        768,
    );
    let k = ui.open_panel(Did(LAYOUT), 0x1).unwrap();
    let vk = ui.find_child(k, 0x2).unwrap();
    ui.set_external_content(vk, Some(77));
    let fr = frames(&mut ui, 3);
    assert!(
        fr.composition
            .entries
            .iter()
            .any(|e| matches!(e.texture, tessera_ui::paint::EntryTexture::External(77))),
        "{:?}",
        fr.composition.entries
    );
    let _ = NodeContent::External(0);
}
