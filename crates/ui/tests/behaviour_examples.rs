// SPDX-License-Identifier: GPL-3.0-or-later
//! Worked examples (§13) and invariants (§14) of `docs/specs/ui-behaviour.md` on synthetic
//! trees. All values are invented.

mod ui_common;

use std::sync::Arc;

use tessera_formats::did::Did;
use tessera_formats::property::PropertyValue;
use tessera_formats::string_info::{StringInfo, StringSource};
use tessera_ui::geom::IBox;
use tessera_ui::{BandChange, ElementKey, ImageRef, MemoryEnv, PreparedText, Ui, UiEvent};
use ui_common::*;

const LAYOUT: u32 = 0x22FF_2000;

fn open(ui: &mut Ui, id: u32) -> ElementKey {
    ui.open_panel(Did(LAYOUT), id).unwrap()
}

fn literal(s: &str) -> PropertyValue {
    PropertyValue::StringInfo(Box::new(StringInfo {
        source: StringSource::Literal(s.into()),
        args: None,
    }))
}

// ----- §13.2 -----------------------------------------------------------------------------------

#[test]
fn example_13_2_hit_test_and_hover() {
    let vv = v();
    let a = z(typed(0xA, vv.types.button, 0, 0, 50, 50), 0, 1);
    let bb = z(field(0xB, 0, 0, 50, 50), -5, 2);
    let p = prop(field(0x1, 0, 0, 100, 100), vv.pids.blocks_clicks, b(true));
    let p = child(child(p, a), bb);
    let mut ui = ui_one(LAYOUT, 1024, 768, vec![p], 1024, 768);
    let pk = open(&mut ui, 0x1);
    let ak = ui.find_child(pk, 0xA).unwrap();
    ui.pointer_moved(10, 10);
    assert_eq!(ui.hit_element(), Some(ak));
    assert_eq!(ui.hovered(), Some(ak));
    ui.set_shown(ak, false);
    ui.pointer_moved(11, 10);
    assert_eq!(ui.hit_element(), Some(pk));
    assert_eq!(ui.hovered(), None);
    // A press on a blocks-clicks-only element is consumed (decision 0014 D7).
    assert!(ui.mouse_press(&cmd("SelectLeft")));
    ui.mouse_release(&cmd("SelectLeft"));
    // Over nothing: not consumed.
    ui.pointer_moved(500, 500);
    assert!(!ui.mouse_press(&cmd("SelectLeft")));
}

// ----- §13.3: tooltip -------------------------------------------------------------------------

#[test]
fn example_13_3_tooltip_size_and_placement() {
    let vv = v();
    let pp = vv.pids;
    let (tt, txt) = (0x1000_0050, 0x1000_0051);
    let tip_layout = 0x22FF_2001;
    let bg = z(
        edges(field(0x1000_0052, 0, 0, 512, 512), [5, 5, 5, 5]),
        1,
        1,
    );
    let text = z(
        edges(typed(txt, vv.types.text, 2, 2, 26, 26), [5, 5, 5, 5]),
        0,
        2,
    );
    let panel = prop(field(tt, 0, 0, 30, 30), pp.tooltip_text_id, en(txt));
    let panel = z(child(child(panel, bg), text), -1500, 0);
    let mut e = field(0x1, 1800, 950, 100, 100);
    e = prop(e, pp.tooltips_on, b(true));
    e = prop(e, pp.tooltip_entry, literal("hello"));
    e = prop(e, pp.tooltip_id, en(tt));
    e = prop(e, pp.tooltip_layout, did(tip_layout));
    let mut env = env_with(vec![
        layout(LAYOUT, 1920, 1080, vec![e]),
        layout(tip_layout, 1024, 768, vec![panel]),
    ]);
    env.texts.insert(
        "hello".into(),
        Arc::new(PreparedText {
            extent: (212, 14),
            ..PreparedText::default()
        }),
    );
    let mut ui = ui_with(env, 1920, 1080);
    let k = open(&mut ui, 0x1);
    assert!(
        ui.mouse_visible(k),
        "tooltip text makes the base element mouse-visible"
    );
    ui.pointer_moved(1850, 1000);
    assert_eq!(ui.hovered(), Some(k));
    ui.set_time(0.2);
    ui.tick_hover();
    assert!(ui.tooltip_panel().is_none());
    ui.set_time(0.3);
    ui.tick_hover();
    let t = ui.tooltip_panel().expect("tooltip shown");
    assert_eq!(ui.layout_box(t), Some(IBox::from_xywh(1704, 1032, 216, 18)));
    // UB-I10.
    let b = ui.layout_box(t).unwrap();
    assert!(b.l <= 1920 - b.w() && b.t <= 1080 - b.h());
    // UB-I13: handled; no second show for this hover.
    ui.set_time(1.0);
    ui.tick_hover();
    assert_eq!(ui.tooltip_panel(), Some(t));
    // The hovered element changes: the tooltip goes.
    ui.pointer_moved(10, 10);
    assert!(ui.tooltip_panel().is_none());
}

// ----- §13.4: drag threshold ------------------------------------------------------------------

#[test]
fn example_13_4_drag_threshold() {
    let vv = v();
    let src = prop(
        typed(0x2, vv.types.button, 50, 50, 100, 100),
        vv.pids.dragable,
        b(true),
    );
    let p = child(field(0x1, 0, 0, 400, 400), z(src, 0, 1));
    let mut ui = ui_one(LAYOUT, 1024, 768, vec![p], 1024, 768);
    open(&mut ui, 0x1);
    ui.pointer_moved(100, 100);
    ui.mouse_press(&cmd("SelectLeft"));
    ui.pointer_moved(103, 102);
    assert!(!ui.item_drag_active());
    ui.pointer_moved(102, 104);
    assert!(ui.item_drag_active());
    assert!(
        ui.take_events()
            .iter()
            .any(|e| matches!(e, UiEvent::DragBegan { .. }))
    );
    // UB-I4: the drag icon is never hit.
    let h = ui.hit_test(102, 104);
    assert!(h.is_none() || !ui.is_created(h.unwrap()) || h != ui.captured());
    ui.mouse_release(&cmd("SelectLeft"));
    assert!(!ui.item_drag_active());
    assert!(
        ui.take_events()
            .iter()
            .any(|e| matches!(e, UiEvent::DragEnded { .. }))
    );
}

// ----- §13.5: resize ----------------------------------------------------------------------------

fn resize_ui(min_height: Option<i32>) -> (Ui, ElementKey) {
    let vv = v();
    let pp = vv.pids;
    let bar = typed(0x2, vv.types.resize_bar, 290, 190, 10, 10);
    let bar = prop(
        prop(bar, pp.resize_border_right, b(true)),
        pp.resize_border_bottom,
        b(true),
    );
    let mut t = field(0x1, 100, 50, 300, 200);
    t = prop(t, pp.min_width, int(200));
    t = prop(t, pp.max_width, int(350));
    t = prop(t, pp.resize_aspect_ratio, f(1.5));
    if let Some(m) = min_height {
        t = prop(t, pp.min_height, int(m));
    }
    let t = child(t, z(bar, 0, 1));
    let mut ui = ui_one(LAYOUT, 1920, 1080, vec![t], 1920, 1080);
    let k = open(&mut ui, 0x1);
    (ui, k)
}

#[test]
fn example_13_5_resize() {
    let (mut ui, k) = resize_ui(None);
    ui.pointer_moved(390, 240);
    ui.mouse_press(&cmd("SelectLeft"));
    ui.pointer_moved(450, 300);
    assert_eq!(ui.layout_box(k), Some(IBox::from_xywh(100, 50, 350, 233)));
    ui.mouse_release(&cmd("SelectLeft"));
    let (mut ui, k) = resize_ui(Some(240));
    ui.pointer_moved(390, 240);
    ui.mouse_press(&cmd("SelectLeft"));
    ui.pointer_moved(450, 300);
    assert_eq!(ui.layout_box(k), Some(IBox::from_xywh(100, 50, 360, 240)));
}

// ----- §13.6: activation and z-order --------------------------------------------------------------

#[test]
fn example_13_6_activation_and_z_order() {
    let vv = v();
    let act = |d| prop(d, vv.pids.activatable, b(true));
    let x = act(typed(0x1, vv.types.button, 0, 0, 100, 100));
    let y = act(typed(0x2, vv.types.button, 50, 50, 100, 100));
    let zz = z(act(typed(0x3, vv.types.button, 300, 300, 100, 100)), -10, 0);
    let mut ui = ui_one(LAYOUT, 1024, 768, vec![x, y, zz], 1024, 768);
    let xk = open(&mut ui, 0x1);
    let yk = open(&mut ui, 0x2);
    let zk = open(&mut ui, 0x3);
    let root = ui.screen_root();
    assert_eq!(ui.children(root), vec![xk, yk, zk]);
    ui.pointer_moved(10, 10);
    ui.mouse_press(&cmd("SelectLeft"));
    ui.mouse_release(&cmd("SelectLeft"));
    assert_eq!(ui.active_panel(), Some(xk));
    assert_eq!(ui.children(root), vec![yk, xk, zk]);
    // UB-I7: z groups keep their stacking.
    ui.activate(zk);
    assert_eq!(ui.children(root), vec![yk, xk, zk]);
}

// ----- §13.7, §13.14, §13.15: scale ------------------------------------------------------------

#[test]
fn example_13_7_panel_scale() {
    let vv = v();
    let pp = vv.pids;
    let p = prop(
        prop(field(0x1, 100, 100, 300, 200), pp.scaler_origin_x, int(150)),
        pp.scaler_origin_y,
        int(100),
    );
    let mut ui = ui_one(LAYOUT, 1920, 1080, vec![p], 1920, 1080);
    let k = open(&mut ui, 0x1);
    ui.scale_panel(k, 1.2, false);
    assert_eq!(ui.layout_box(k), Some(IBox::from_xywh(70, 80, 360, 240)));
}

fn scaled_ui() -> (Ui, ElementKey, ElementKey) {
    let vv = v();
    let pp = vv.pids;
    let c = typed(0x2, vv.types.button, 10, 20, 50, 10);
    let c = prop(
        media(c, image_item(0x4100_0001, 4)),
        pp.alpha_threshold,
        int(128),
    );
    let p = prop(
        prop(field(0x1, 100, 100, 300, 200), pp.scaler_origin_x, int(150)),
        pp.scaler_origin_y,
        int(100),
    );
    let p = child(p, z(c, 0, 1));
    let mut env = env_with(vec![layout(LAYOUT, 1920, 1080, vec![p])]);
    let mut px = vec![0xFF00_0000u32; 60 * 12];
    px[5 * 60 + 6] = 0x0000_0000;
    let img = tessera_material::ui_image::UiImage::new(
        60,
        12,
        60,
        12,
        tessera_material::ui_image::PixelLayout::A8R8G8B8,
        px,
    )
    .unwrap();
    env.images.insert(ImageRef::Did(0x4100_0001), Arc::new(img));
    let mut ui = ui_with(env, 1920, 1080);
    let k = open(&mut ui, 0x1);
    let ck = ui.find_child(k, 0x2).unwrap();
    ui.scale_panel(k, 1.2, true);
    (ui, k, ck)
}

#[test]
fn example_13_14_displayed_box_of_a_scaled_panel() {
    let (ui, k, ck) = scaled_ui();
    assert_eq!(ui.layout_box(k), Some(IBox::new(70, 80, 369, 279)));
    assert_eq!(ui.displayed_box(k), IBox::new(70, 80, 429, 319));
    assert_eq!(ui.displayed_rel(ck), IBox::from_xywh(12, 24, 60, 12));
    assert!(ui.displayed_box(ck).contains(90, 110));
    // UB-I20: the child's scale is the panel's.
    assert_eq!(ui.node_scale_at_or_above(ck), (1.2, 1.2, 1.0));
}

#[test]
fn example_13_15_alpha_test_point_in_a_scaled_panel() {
    let (mut ui, k, ck) = scaled_ui();
    // (20, 30) relative to the panel's displayed box is image point (6, 5): alpha 0.
    let h = ui.hit_test(70 + 20, 80 + 30);
    assert_ne!(h, Some(ck));
    let _ = k;
    // (21, 30) → (8, 5)... any opaque point hits the child.
    let h = ui.hit_test(70 + 30, 80 + 30);
    assert_eq!(h, Some(ck));
}

// ----- §13.9: input maps and focus ------------------------------------------------------------

#[test]
fn example_13_9_input_maps_and_focus() {
    let vv = v();
    let f1 = field(0x2, 0, 0, 10, 10);
    let p = prop(
        child(field(0x1, 0, 0, 100, 100), z(f1, 0, 1)),
        vv.pids.activatable,
        b(true),
    );
    let mut ui = ui_one(LAYOUT, 1024, 768, vec![p], 1024, 768);
    let pk = open(&mut ui, 0x1);
    let fk = ui.find_child(pk, 0x2).unwrap();
    ui.set_element_maps(pk, vec!["MP".into()]);
    ui.set_element_maps(fk, vec!["MF".into()]);
    ui.take_band_changes();
    ui.activate(pk);
    let band = |ui: &Ui| -> Vec<(String, ElementKey, i32)> {
        ui.band()
            .iter()
            .map(|e| (e.map.clone(), e.owner, e.priority))
            .collect()
    };
    assert_eq!(band(&ui), vec![("MP".into(), pk, 0)]);
    ui.set_focus(Some(fk));
    assert_eq!(
        band(&ui),
        vec![
            ("MF".into(), fk, 50),
            ("MP".into(), pk, 49),
            ("MP".into(), pk, 0)
        ]
    );
    // UB-I12: the focus element's map is above the active panel's.
    ui.set_focus(None);
    assert_eq!(band(&ui), vec![("MP".into(), pk, 0)]);
    let ch = ui.take_band_changes();
    assert!(ch.contains(&BandChange::Pop {
        map: "MF".into(),
        owner: fk.owner()
    }));
    assert!(ch.iter().all(|c| match c {
        BandChange::Push { owner, .. } | BandChange::Pop { owner, .. } => *owner != 0,
    }));
}

// ----- §13.10: move clamp ---------------------------------------------------------------------

#[test]
fn example_13_10_move_clamp() {
    let vv = v();
    let bar = typed(0x2, vv.types.dragbar, 0, 0, 10, 10);
    let p = child(field(0x1, 0, 0, 1000, 400), z(bar, 0, 1));
    let mut ui = ui_one(LAYOUT, 800, 600, vec![p], 800, 600);
    let k = open(&mut ui, 0x1);
    ui.pointer_moved(5, 5);
    ui.mouse_press(&cmd("SelectLeft"));
    ui.pointer_moved(105, 305);
    let b = ui.layout_box(k).unwrap();
    assert_eq!((b.l, b.t), (-200, 200));
    // UB-I8.
    assert_eq!(b.r, 799);
    assert!(b.t >= 0 && b.b <= 599);
    ui.mouse_release(&cmd("SelectLeft"));
}

// ----- §13.11: the mouse-visible flag ------------------------------------------------------------

#[test]
fn example_13_11_mouse_visible_flag() {
    let vv = v();
    let pp = vv.pids;
    let bk = prop(
        prop(
            typed(0xB, vv.types.button, 0, 0, 10, 10),
            pp.tooltips_on,
            b(true),
        ),
        pp.tooltip_entry,
        literal("t"),
    );
    let fk = field(0xF, 20, 0, 10, 10);
    let gk = prop(field(0x6, 40, 0, 10, 10), pp.context_menu, b(true));
    let p = child(
        child(child(field(0x1, 0, 0, 100, 100), z(bk, 0, 1)), z(fk, 0, 2)),
        z(gk, 0, 3),
    );
    let mut ui = ui_one(LAYOUT, 1024, 768, vec![p], 1024, 768);
    let k = open(&mut ui, 0x1);
    let (b1, f1, g1) = (
        ui.find_child(k, 0xB).unwrap(),
        ui.find_child(k, 0xF).unwrap(),
        ui.find_child(k, 0x6).unwrap(),
    );
    assert!(ui.mouse_visible(b1) && !ui.mouse_visible(f1) && ui.mouse_visible(g1));
    assert_eq!(ui.hit_test(25, 5), None);
    ui.register_handler(0xF, vv.msgs.mouse_click);
    assert!(ui.mouse_visible(f1));
    assert_eq!(ui.hit_test(25, 5), Some(f1));
    ui.set_property(f1, pp.context_menu, b(false));
    assert!(!ui.mouse_visible(f1));
    ui.set_property(b1, pp.button_ghosted, b(true));
    assert!(!ui.mouse_visible(b1));
}

// ----- §13.12: enter and leave chains -------------------------------------------------------------

#[test]
fn example_13_12_enter_and_leave_chains() {
    let vv = v();
    let bt = vv.types.button;
    let a1 = z(typed(0xA1, bt, 0, 0, 50, 50), 0, 1);
    let a = child(z(typed(0xA, bt, 0, 0, 100, 100), 0, 1), a1);
    let c = z(typed(0xC, bt, 120, 120, 50, 50), 0, 2);
    let p = prop(
        child(child(field(0x1, 0, 0, 200, 200), a), c),
        vv.pids.context_menu,
        b(true),
    );
    let mut ui = ui_one(LAYOUT, 1024, 768, vec![p], 1024, 768);
    let root_id = vv.ids.screen_root;
    for id in [0x1, 0xA, 0xA1, 0xC, root_id] {
        ui.register_handler(id, vv.msgs.mouse_over);
    }
    open(&mut ui, 0x1);
    let seq = |ui: &mut Ui, x, y| -> Vec<(u32, i64)> {
        ui.take_events();
        ui.pointer_moved(x, y);
        ui.take_events()
            .into_iter()
            .filter_map(|e| match e {
                UiEvent::Game { message } if message.msg == vv.msgs.mouse_over => {
                    Some((message.source_id, message.arg.value))
                }
                _ => None,
            })
            .collect()
    };
    assert_eq!(
        seq(&mut ui, 10, 10),
        vec![(0xA1, 1), (0xA, 1), (0x1, 1), (root_id, 1)]
    );
    // UB-I17: leaves before enters; nothing above the common ancestor.
    assert_eq!(seq(&mut ui, 130, 130), vec![(0xA1, 0), (0xA, 0), (0xC, 1)]);
    assert_eq!(seq(&mut ui, 70, 70), vec![(0xC, 0), (0xA, 1)]);
    assert_eq!(seq(&mut ui, 150, 10), vec![(0xA, 0), (0x1, 0), (0x1, 1)]);
}

// ----- §13.13: wheel over a list --------------------------------------------------------------

#[test]
fn example_13_13_wheel_over_a_list() {
    let vv = v();
    let pp = vv.pids;
    let bt = vv.types.button;
    let (q, qb, l, lb, e, t) = (0x10, 0x11, 0x20, 0x21, 0x30, 0x31);
    let bar = |id: u32, x: i32, h: i32| {
        let b1 = typed(id, vv.types.scrollbar, x, 0, 10, h);
        let b1 = prop(b1, pp.sb_proportional, b(true));
        child(b1, z(typed(vv.ids.widget, bt, 0, 0, 10, 10), 0, 1))
    };
    let qd = prop(
        prop(
            typed(q, vv.types.list_box, 0, 0, 300, 200),
            pp.scrollable_vbar,
            en(qb),
        ),
        pp.lb_max_columns,
        int(1),
    );
    let root = child(
        child(field(0x1, 0, 0, 600, 600), z(qd, 0, 1)),
        z(bar(qb, 300, 200), 0, 2),
    );
    let ld = prop(
        prop(
            typed(l, vv.types.list_box, 0, 0, 280, 300),
            pp.scrollable_vbar,
            en(lb),
        ),
        pp.lb_max_columns,
        int(1),
    );
    let ld = child(ld, z(bar(lb, 270, 300), -1, 1));
    let td = prop(
        typed(t, vv.types.text, 0, 0, 100, 20),
        pp.text_selectable,
        b(true),
    );
    let ed = child(field(e, 0, 0, 260, 40), z(td, 0, 1));
    let mut ui = ui_one(LAYOUT, 1024, 768, vec![root, ld, ed], 1024, 768);
    let rk = open(&mut ui, 0x1);
    let qk = ui.find_child(rk, q).unwrap();
    let lk = ui.create_element(Some(qk), Did(LAYOUT), l).unwrap();
    ui.list_add(qk, lk, None).unwrap();
    for _ in 0..10 {
        let ek = ui.create_element(Some(lk), Did(LAYOUT), e).unwrap();
        ui.list_add(lk, ek, None).unwrap();
    }
    ui.pointer_moved(10, 5);
    let hit = ui.hit_element().unwrap();
    assert_eq!(ui.element_id(hit), Some(t));
    assert!(ui.wheel(&cmd("ScrollDown")));
    assert_eq!(ui.scroll_state(lk).unwrap().1, (0, 40));
    assert_eq!(ui.scroll_state(qk).unwrap().1, (0, 0));
    // With L's bar hidden, Q scrolls instead.
    ui.scroll_to(lk, 0, 0, false);
    let lbk = ui.find_child(lk, lb).unwrap();
    ui.set_shown(lbk, false);
    ui.pointer_moved(11, 5);
    assert!(ui.wheel(&cmd("ScrollDown")));
    assert_eq!(ui.scroll_state(lk).unwrap().1, (0, 0));
    assert_ne!(ui.scroll_state(qk).unwrap().1, (0, 0));
}

// ----- invariants -------------------------------------------------------------------------------

#[test]
fn ub_i5_i6_capture_and_clicks() {
    let vv = v();
    let bt = vv.types.button;
    let a = z(typed(0xA, bt, 0, 0, 50, 50), 0, 1);
    let c = z(typed(0xC, bt, 100, 0, 50, 50), 0, 2);
    let p = child(child(field(0x1, 0, 0, 200, 200), a), c);
    let mut ui = ui_one(LAYOUT, 1024, 768, vec![p], 1024, 768);
    ui.register_handler(0xA, vv.msgs.mouse_click);
    ui.register_handler(0xC, vv.msgs.mouse_click);
    let k = open(&mut ui, 0x1);
    let ak = ui.find_child(k, 0xA).unwrap();
    ui.pointer_moved(10, 10);
    ui.mouse_press(&cmd("SelectLeft"));
    assert_eq!(ui.captured(), Some(ak));
    ui.pointer_moved(110, 10);
    // UB-I5.
    assert!(ui.hovered().is_none() || ui.hovered() == ui.captured());
    ui.take_events();
    ui.mouse_release(&cmd("SelectLeft"));
    // UB-I6: released over another element: no click.
    let clicks = ui
        .take_events()
        .into_iter()
        .filter(|e| matches!(e, UiEvent::Game { message } if message.msg == vv.msgs.mouse_click))
        .count();
    assert_eq!(clicks, 0);
    // Pressed and released over the same element: one click.
    ui.mouse_press(&cmd("SelectLeft"));
    ui.mouse_release(&cmd("SelectLeft"));
    let clicks: Vec<u32> = ui
        .take_events()
        .into_iter()
        .filter_map(|e| match e {
            UiEvent::Game { message } if message.msg == vv.msgs.mouse_click => {
                Some(message.source_id)
            }
            _ => None,
        })
        .collect();
    assert_eq!(clicks, vec![0xC]);
}

#[test]
fn ub_i18_stop_ends_the_walk() {
    let vv = v();
    let bt = vv.types.button;
    let mut g = typed(0x5, vv.types.group_box, 0, 0, 300, 50);
    g = prop(g, vv.pids.gb_default, en(0xC));
    g = child(g, z(typed(0xC, bt, 0, 0, 50, 20), 0, 1));
    let p = child(field(0x1, 0, 0, 400, 400), z(g, 0, 1));
    let mut ui = ui_one(LAYOUT, 1024, 768, vec![p], 1024, 768);
    ui.register_handler(0xC, vv.msgs.button_click);
    open(&mut ui, 0x1);
    ui.pointer_moved(5, 5);
    ui.take_events();
    ui.mouse_press(&cmd("SelectLeft"));
    ui.mouse_release(&cmd("SelectLeft"));
    assert!(
        !ui.take_events()
            .iter()
            .any(|e| matches!(e, UiEvent::Game { message } if message.msg == vv.msgs.button_click))
    );
}

#[test]
fn auto_fade_hides_after_the_fade_and_is_not_hit_while_fading() {
    let vv = v();
    let p = prop(
        typed(0x1, vv.types.button, 0, 0, 100, 100),
        vv.pids.auto_fade,
        f(0.1),
    );
    let mut ui = ui_one(LAYOUT, 1024, 768, vec![p], 1024, 768);
    let k = open(&mut ui, 0x1);
    ui.set_shown(k, false);
    assert!(ui.is_visible(k) && ui.is_fading(k));
    assert_eq!(ui.hit_test(5, 5), None);
    ui.set_time(0.05);
    ui.frame_work();
    let o = ui.prop(k, vv.pids.object_opacity).float();
    assert!((o - 0.5).abs() < 1e-3);
    ui.set_time(0.2);
    ui.frame_work();
    assert!(!ui.is_visible(k) && !ui.is_fading(k));
    ui.set_shown(k, true);
    assert!(ui.is_visible(k) && ui.is_fading(k));
}

#[test]
fn dialog_keys_close_with_results() {
    let vv = v();
    let d = prop(field(0x1, 0, 0, 100, 100), vv.pids.activatable, b(true));
    let mut ui = ui_one(LAYOUT, 1024, 768, vec![d], 1024, 768);
    let k = open(&mut ui, 0x1);
    ui.mark_dialog(k);
    ui.activate(k);
    assert!(
        ui.band()
            .iter()
            .any(|e| e.map == "DialogBoxes" && e.owner == k)
    );
    assert!(ui.key_command(&cmd("EscapeKey"), true, 1.0));
    assert!(ui.take_events().contains(&UiEvent::DialogResult {
        dialog: k,
        yes: false
    }));
    assert!(!ui.is_visible(k));
    // The release of the press that gave the result is consumed (`ui-widgets.md` §18.4).
    assert!(ui.key_command(&cmd("EscapeKey"), false, 0.0));
    // A new press with no keyboard target goes on to game code. (A second press without
    // the release would be an OS repeat of the first, consumed with it.)
    assert!(!ui.key_command(&cmd("EscapeKey"), true, 1.0));
    assert!(
        ui.take_events()
            .iter()
            .any(|e| matches!(e, UiEvent::KeyBroadcast { down: true, .. }))
    );
}

#[test]
fn editable_text_takes_characters_and_pushes_edit_maps() {
    let vv = v();
    let pp = vv.pids;
    let t = prop(
        prop(
            typed(0x2, vv.types.text, 0, 0, 100, 20),
            pp.text_editable,
            b(true),
        ),
        pp.text_lose_focus_on_accept,
        b(true),
    );
    let p = prop(
        child(field(0x1, 0, 0, 200, 200), z(t, 0, 1)),
        pp.activatable,
        b(true),
    );
    let mut ui = ui_one(LAYOUT, 1024, 768, vec![p], 1024, 768);
    let k = open(&mut ui, 0x1);
    let tk = ui.find_child(k, 0x2).unwrap();
    ui.activate(k);
    ui.set_focus(Some(tk));
    assert!(
        ui.band()
            .iter()
            .any(|e| e.map == "EditControls" && e.owner == tk && e.priority == 50)
    );
    assert!(
        ui.band()
            .iter()
            .any(|e| e.map == "CopyAndPasteControls" && e.owner == tk)
    );
    for c in "abc".chars() {
        assert!(ui.text_input(c));
    }
    assert_eq!(ui.text_of(tk), "abc");
    assert!(ui.key_command(&cmd("BackspaceKey"), true, 1.0));
    assert_eq!(ui.text_of(tk), "ab");
    assert!(ui.key_command(&cmd("AcceptInput"), true, 1.0));
    assert_eq!(ui.focus_element(), None);
    assert!(!ui.band().iter().any(|e| e.map == "EditControls"));
}

#[test]
fn pointer_lock_hovers_nothing_and_releases_without_click() {
    let vv = v();
    let a = typed(0xA, vv.types.button, 0, 0, 50, 50);
    let mut ui = ui_one(LAYOUT, 1024, 768, vec![a], 1024, 768);
    ui.register_handler(0xA, vv.msgs.mouse_click);
    ui.register_handler(0xA, vv.msgs.mouse_release);
    let k = open(&mut ui, 0xA);
    ui.pointer_moved(10, 10);
    ui.mouse_press(&cmd("SelectLeft"));
    ui.set_pointer_lock(Some((10, 10)));
    assert_eq!(ui.hovered(), None);
    assert!(!ui.mouse_press(&cmd("SelectRight")));
    ui.take_events();
    ui.mouse_release(&cmd("SelectLeft"));
    let ev = ui.take_events();
    let has = |m: u32| {
        ev.iter()
            .any(|e| matches!(e, UiEvent::Game { message } if message.msg == m))
    };
    assert!(has(vv.msgs.mouse_release));
    assert!(!has(vv.msgs.mouse_click));
    assert_eq!(ui.captured(), None);
    ui.set_pointer_lock(None);
    ui.pointer_moved(10, 11);
    assert_eq!(ui.hovered(), Some(k));
}

#[test]
fn ub_i19_no_node_displayed_equals_layout() {
    let mut ui = ui_one(
        LAYOUT,
        1024,
        768,
        vec![child(
            field(0x1, 10, 20, 100, 100),
            z(field(0x2, 5, 5, 10, 10), 0, 1),
        )],
        1024,
        768,
    );
    let k = open(&mut ui, 0x1);
    let c = ui.find_child(k, 0x2).unwrap();
    assert_eq!(ui.displayed_rel(c), ui.layout_box(c).unwrap());
    assert_eq!(ui.displayed_box(c), IBox::from_xywh(15, 25, 10, 10));
}

#[test]
fn memory_env_is_usable_without_layouts() {
    let mut ui = ui_with(MemoryEnv::default(), 0, 0);
    assert_eq!(ui.screen_size(), (1024, 768));
    assert!(ui.open_panel(Did(LAYOUT), 1).is_err());
    assert!(ui.open_panel_by_key(5, 1).is_err());
}

#[test]
fn activate_on_show_activates_an_opened_panel() {
    let vv = v();
    let p = prop(
        prop(field(0x1, 0, 0, 50, 50), vv.pids.activate_on_show, b(true)),
        vv.pids.activatable,
        b(false),
    );
    let mut ui = ui_one(LAYOUT, 1024, 768, vec![p], 1024, 768);
    let k = open(&mut ui, 0x1);
    assert_eq!(ui.active_panel(), Some(k));
    // A request by code makes the panel activatable (§5.1 step 2).
    assert!(ui.prop(k, vv.pids.activatable).bool());
}
