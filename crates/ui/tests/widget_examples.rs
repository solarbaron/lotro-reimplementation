// SPDX-License-Identifier: GPL-3.0-or-later
//! Worked examples (§15) and invariants (§14) of `docs/specs/ui-widgets.md` on synthetic
//! trees. All values are invented.

mod ui_common;

use proptest::prelude::*;
use tessera_formats::did::Did;
use tessera_ui::geom::IBox;
use tessera_ui::widgets::meter::{child_fill, frame_index};
use tessera_ui::widgets::scroll::{stop_to_value, value_to_stop};
use tessera_ui::{ElementKey, ImageRef, Ui, UiEvent};
use ui_common::*;

const LAYOUT: u32 = 0x22FF_1000;

fn open(ui: &mut Ui, id: u32) -> ElementKey {
    ui.open_panel(Did(LAYOUT), id).unwrap()
}

// ----- §15.1, §15.2: meters --------------------------------------------------------------

#[test]
fn example_15_1_child_meter() {
    let cases = [
        (4, false, 0.75, [0, 0, 123, 9], [0, 0, 123, 9]),
        (3, false, 0.75, [41, 0, 123, 9], [-41, 0, 164, 9]),
        (4, true, 0.75, [-41, 0, 164, 9], [0, 0, 164, 9]),
        (4, false, 1.0, [0, 0, 164, 9], [0, 0, 164, 9]),
    ];
    for (dir, mv, p, fill, img) in cases {
        assert_eq!(
            child_fill(dir, mv, p, 0, 164, 9),
            (fill, img),
            "{dir} {mv} {p}"
        );
    }
    // Through the tree: parts created at post-creation, updated on position changes.
    let vv = v();
    let pp = vv.pids;
    let m = typed(0x1000_0001, vv.types.meter, 0, 0, 164, 9);
    let m = prop(m, pp.meter_child, b(true));
    let m = prop(m, pp.meter_direction, en(3));
    let m = prop(m, pp.meter_child_image, did(0x4100_0100));
    let m = prop(m, pp.meter_child_drawmode, en(2));
    let m = prop(m, pp.meter_position, f(0.75));
    let mut ui = ui_one(LAYOUT, 200, 200, vec![m], 200, 200);
    let k = open(&mut ui, 0x1000_0001);
    let fill = ui.find_child(k, vv.ids.meter_fill).unwrap();
    let image = ui.find_child(fill, vv.ids.meter_fill_image).unwrap();
    assert_eq!(ui.layout_box(fill), Some(IBox::from_xywh(41, 0, 123, 9)));
    assert_eq!(ui.layout_box(image), Some(IBox::from_xywh(-41, 0, 164, 9)));
    assert_eq!(ui.base_image(image), Some((ImageRef::Did(0x4100_0100), 2)));
    ui.set_property(k, pp.meter_position, f(1.0));
    assert_eq!(ui.layout_box(fill), Some(IBox::from_xywh(0, 0, 164, 9)));
    // WI3: the image never moves relative to the meter.
    assert_eq!(
        ui.layout_box(image).unwrap().l + ui.layout_box(fill).unwrap().l,
        0
    );
    // A state that sets the child image re-sets the fill image (§4.3).
    ui.set_property(k, pp.meter_child_image, did(0x4100_0101));
    assert_eq!(ui.base_image(image), Some((ImageRef::Did(0x4100_0101), 2)));
}

#[test]
fn example_15_2_frame_meter() {
    assert_eq!(frame_index(60, 0.5), 30);
    assert_eq!(frame_index(60, 0.991), 58);
    assert_eq!(frame_index(60, 1.0), 59);
    let vv = v();
    let pp = vv.pids;
    let frames: Vec<_> = (0..60).map(|i| did(0x4100_0200 + i)).collect();
    let m = typed(0x1000_0001, vv.types.meter, 0, 0, 20, 20);
    let m = prop(m, pp.meter_frame, b(true));
    let m = prop(m, pp.meter_frame_array, array(0x473, frames));
    let m = prop(m, pp.meter_position, f(0.5));
    let mut ui = ui_one(LAYOUT, 200, 200, vec![m], 200, 200);
    let k = open(&mut ui, 0x1000_0001);
    assert_eq!(ui.base_image(k), Some((ImageRef::Did(0x4100_0200 + 30), 2)));
    ui.set_property(k, pp.meter_position, f(0.991));
    assert_eq!(ui.base_image(k), Some((ImageRef::Did(0x4100_0200 + 58), 2)));
}

#[test]
fn meter_messages_and_smooth_movement() {
    let vv = v();
    let pp = vv.pids;
    let m = typed(0x1000_0001, vv.types.meter, 0, 0, 20, 20);
    let m = prop(m, pp.meter_notify, b(true));
    let m = prop(m, pp.meter_smooth, b(true));
    let m = prop(m, pp.meter_smooth_duration, f(1.0));
    let mut ui = ui_one(LAYOUT, 200, 200, vec![m], 200, 200);
    ui.register_handler(0x1000_0001, vv.msgs.meter_new_position);
    ui.register_handler(0x1000_0001, vv.msgs.meter_goal_reached);
    let k = open(&mut ui, 0x1000_0001);
    ui.take_events();
    ui.set_property(k, pp.meter_goal, f(1.0));
    ui.set_time(0.5);
    ui.frame_work();
    assert!((ui.prop(k, pp.meter_position).float() - 0.5).abs() < 1e-5);
    ui.set_time(1.5);
    ui.frame_work();
    assert_eq!(ui.prop(k, pp.meter_position).float(), 1.0);
    let ev = ui.take_events();
    let count = |m: u32| {
        ev.iter()
            .filter(|e| matches!(e, UiEvent::Game { message } if message.msg == m))
            .count()
    };
    assert_eq!(count(vv.msgs.meter_new_position), 2);
    assert_eq!(count(vv.msgs.meter_goal_reached), 1);
    // Smooth movement without a duration ignores the goal (§4.6).
    ui.clear_property(k, pp.meter_smooth_duration);
    ui.set_property(k, pp.meter_smooth_duration, f(0.0));
    ui.set_property(k, pp.meter_goal, f(0.2));
    ui.set_time(5.0);
    ui.frame_work();
    assert_eq!(ui.prop(k, pp.meter_position).float(), 1.0);
}

// ----- §15.3: scroll bar and list -----------------------------------------------------------

const LIST: u32 = 0x1000_0010;
const BAR: u32 = 0x1000_0011;
const DEC: u32 = 0x1000_0012;
const INC: u32 = 0x1000_0013;
const ENTRY: u32 = 0x1000_0020;

fn scroll_ui() -> (Ui, ElementKey, ElementKey) {
    let vv = v();
    let pp = vv.pids;
    let bt = vv.types.button;
    let bar = typed(BAR, vv.types.scrollbar, 430, 0, 10, 447);
    let bar = prop(bar, pp.sb_proportional, b(true));
    let bar = prop(bar, pp.sb_min_widget_size, int(20));
    let bar = prop(bar, pp.sb_dec_button, en(DEC));
    let bar = prop(bar, pp.sb_inc_button, en(INC));
    let bar = prop(bar, pp.sb_hide_when_disabled, b(true));
    let bar = child(bar, z(typed(vv.ids.widget, bt, 0, 0, 10, 10), 0, 1));
    let bar = child(bar, z(typed(DEC, bt, 0, 0, 10, 10), 0, 2));
    let bar = child(bar, z(typed(INC, bt, 0, 437, 10, 10), 0, 3));
    let list = typed(LIST, vv.types.list_box, 0, 0, 419, 433);
    let list = prop(list, pp.lb_max_columns, int(1));
    let list = prop(list, pp.scrollable_vbar, en(BAR));
    let root = child(
        child(field(0x1000_0001, 0, 0, 500, 500), z(list, 0, 1)),
        z(bar, 0, 2),
    );
    let entry = field(ENTRY, 0, 0, 100, 30);
    let mut ui = ui_one(LAYOUT, 1024, 768, vec![root, entry], 1024, 768);
    let r = open(&mut ui, 0x1000_0001);
    let l = ui.find_child(r, LIST).unwrap();
    let bk = ui.find_child(r, BAR).unwrap();
    (ui, l, bk)
}

#[test]
fn example_15_3_vertical_bar_and_list() {
    let (mut ui, l, bar) = scroll_ui();
    let vv = v();
    let pp = vv.pids;
    // WI16: an empty list leaves its bar disabled and hidden.
    assert!(ui.prop(bar, pp.sb_disabled).bool());
    assert!(!ui.is_visible(bar));
    for _ in 0..40 {
        let e = ui.create_element(Some(l), Did(LAYOUT), ENTRY).unwrap();
        ui.list_add(l, e, None).unwrap();
    }
    assert_eq!(ui.scroll_state(l), Some(((100, 1200), (0, 0))));
    assert!(ui.is_visible(bar));
    let ws = ui.prop(bar, pp.sb_widget_size).float();
    assert!((ws - 433.0 / 1200.0).abs() < 1e-6);
    let thumb = ui.find_child(bar, vv.ids.widget).unwrap();
    assert_eq!(ui.layout_box(thumb), Some(IBox::from_xywh(0, 10, 10, 154)));
    // WI10: entry k at y = 30 k.
    let entries = ui.list_entries(l);
    assert_eq!(ui.layout_box(entries[3]).unwrap().t, 90);
    // One ScrollDown over the list: one line step of 30.
    ui.pointer_moved(50, 50);
    assert!(ui.wheel(&cmd("ScrollDown")));
    assert_eq!(ui.scroll_state(l).unwrap().1, (0, 30));
    let q = ui.prop(bar, pp.sb_position).float();
    assert!((q - 30.0 / 767.0).abs() < 1e-6, "{q}");
    assert_eq!(ui.layout_box(entries[3]).unwrap().t, 60);
    // Dragging the thumb 50 px down from q = 0.
    ui.scroll_to(l, 0, 0, false);
    let tb = ui.displayed_box(thumb);
    ui.pointer_moved(tb.l + 2, tb.t + 2);
    ui.mouse_press(&cmd("SelectLeft"));
    ui.pointer_moved(tb.l + 2, tb.t + 52);
    let q = ui.prop(bar, pp.sb_position).float();
    assert!((q - 50.0 / 273.0).abs() < 1e-6, "{q}");
    assert_eq!(ui.scroll_state(l).unwrap().1, (0, 140));
    ui.mouse_release(&cmd("SelectLeft"));
    // Page and line from the arrow buttons.
    ui.scroll_to(l, 0, 0, false);
    let inc = ui.find_child(bar, INC).unwrap();
    let ib = ui.displayed_box(inc);
    ui.pointer_moved(ib.l + 1, ib.t + 1);
    ui.mouse_press(&cmd("SelectLeft"));
    ui.mouse_release(&cmd("SelectLeft"));
    assert_eq!(ui.scroll_state(l).unwrap().1, (0, 30));
}

#[test]
fn example_15_4_stops() {
    let n = Some(100);
    let i = value_to_stop(n, false, 0.37).unwrap();
    assert_eq!(i, 37);
    assert!((stop_to_value(n, false, i) - 37.0 / 99.0).abs() < 1e-7);
    let i = value_to_stop(Some(4), true, 0.6).unwrap();
    assert_eq!(i, 2);
    assert_eq!(stop_to_value(Some(4), true, i), 0.625);
    assert_eq!(value_to_stop(None, false, 0.5), None);
    assert_eq!(stop_to_value(None, false, 3), -1.0);
}

proptest! {
    /// WI5: value-to-stop and stop-to-value are inverse on stops.
    #[test]
    fn wi5_stops_inverse(n in 2i32..400, no_end in any::<bool>(), i in 0i32..400) {
        let i = i % n;
        let q = stop_to_value(Some(n), no_end, i);
        prop_assert_eq!(value_to_stop(Some(n), no_end, q), Some(i));
    }

    /// WI2, WI3: the fill of a child meter lies inside the meter and the image stays put.
    #[test]
    fn wi2_wi3_child_meter(dir in 3u32..5, p in 0.0f32..=1.0, w in 1i32..2000) {
        let (fl, im) = child_fill(dir, false, p, 0, w, 9);
        prop_assert!(fl[0] >= 0 && fl[0] + fl[2] <= w);
        prop_assert_eq!(fl[2], (w as f32 * p) as i32);
        if dir == 4 { prop_assert_eq!(fl[0], 0); } else { prop_assert_eq!(fl[0] + fl[2], w); }
        prop_assert_eq!(fl[0] + im[0], 0);
    }

    /// WI4: the frame index is in range.
    #[test]
    fn wi4_frame_index(n in 1usize..200, p in 0.0f32..=1.0) {
        let i = frame_index(n, p);
        prop_assert!(i >= 0 && (i as usize) < n);
    }
}

// ----- §15.5: horizontal list with a row cap -------------------------------------------------

#[test]
fn example_15_5_row_cap() {
    let vv = v();
    let pp = vv.pids;
    let list = typed(LIST, vv.types.list_box, 0, 0, 500, 100);
    let list = prop(list, pp.lb_horizontal, b(true));
    let list = prop(list, pp.lb_max_columns, int(12));
    let list = prop(list, pp.lb_max_rows, int(1));
    let root = child(field(0x1000_0001, 0, 0, 600, 600), z(list, 0, 1));
    let entry = field(ENTRY, 0, 0, 35, 35);
    let mut ui = ui_one(LAYOUT, 1024, 768, vec![root, entry], 1024, 768);
    let r = open(&mut ui, 0x1000_0001);
    let l = ui.find_child(r, LIST).unwrap();
    for _ in 0..14 {
        let e = ui.create_element(Some(l), Did(LAYOUT), ENTRY).unwrap();
        ui.move_to(e, 999, 999);
        ui.list_add(l, e, None).unwrap();
    }
    let es = ui.list_entries(l);
    for (i, e) in es.iter().enumerate().take(12) {
        assert_eq!(ui.layout_box(*e).unwrap().l, 35 * i as i32);
        assert_eq!(ui.layout_box(*e).unwrap().t, 0);
    }
    assert_eq!(ui.layout_box(es[12]).unwrap().l, 999);
    assert_eq!(ui.list_grid(l), Some((12, 1)));
    assert_eq!(ui.scroll_state(l).unwrap().0, (420, 35));
}

// ----- §15.6: tabs -----------------------------------------------------------------------------

#[test]
fn example_15_6_tabs() {
    let vv = v();
    let pp = vv.pids;
    let st = vv.states;
    let (ta, tb, tc) = (0x1000_00A1, 0x1000_00A2, 0x1000_00A3);
    let (pa, pb, pc) = (0x1000_00B1, 0x1000_00B2, 0x1000_00B3);
    let page = |tab: u32, page: u32, open: bool| {
        strukt(vec![
            (pp.panel_tab, en(tab)),
            (pp.panel_page, en(page)),
            (pp.panel_page_open, b(open)),
        ])
    };
    let mut panel = typed(0x1000_0001, vv.types.panel, 0, 0, 400, 300);
    panel = prop(
        panel,
        pp.panel_pages,
        array(
            0x21,
            vec![page(ta, pa, false), page(tb, pb, true), page(tc, pc, true)],
        ),
    );
    for (i, t) in [ta, tb, tc].into_iter().enumerate() {
        let btn = typed(t, vv.types.button, i as i32 * 50, 0, 40, 20);
        let btn = state(
            state(btn, st.normal, 0, vec![], vec![]),
            st.highlight,
            0,
            vec![],
            vec![],
        );
        panel = child(panel, z(btn, 0, i as u32 + 1));
    }
    for (i, pg) in [pa, pb, pc].into_iter().enumerate() {
        panel = child(panel, z(field(pg, 0, 30, 400, 270), 0, i as u32 + 4));
    }
    let mut ui = ui_one(LAYOUT, 1024, 768, vec![panel], 1024, 768);
    let k = open(&mut ui, 0x1000_0001);
    let get = |ui: &Ui, id| ui.find_child(k, id).unwrap();
    assert_eq!(ui.panel_current(k), Some((pc, tc)));
    assert!(ui.is_visible(get(&ui, pc)));
    assert!(!ui.is_visible(get(&ui, pa)) && !ui.is_visible(get(&ui, pb)));
    assert!(ui.prop(get(&ui, tc), pp.button_highlighted).bool());
    assert!(!ui.prop(get(&ui, ta), pp.button_highlighted).bool());
    // WI11.
    let shown = [pa, pb, pc]
        .iter()
        .filter(|p| ui.is_visible(get(&ui, **p)))
        .count();
    assert_eq!(shown, 1);
    // A click on A opens (a, A).
    ui.pointer_moved(5, 5);
    ui.mouse_press(&cmd("SelectLeft"));
    ui.mouse_release(&cmd("SelectLeft"));
    assert_eq!(ui.panel_current(k), Some((pa, ta)));
    assert!(ui.prop(get(&ui, ta), pp.button_highlighted).bool());
    assert!(!ui.prop(get(&ui, tc), pp.button_highlighted).bool());
    assert_eq!(ui.state(get(&ui, ta)), Some(st.highlight));
}

// ----- §15.7: state requests -----------------------------------------------------------------

#[test]
fn example_15_7_and_wi12_state_requests() {
    let vv = v();
    let pp = vv.pids;
    let st = vv.states;
    // (S, start (H, G), request, result)
    let cases = [
        (false, (false, false), st.highlight, (true, false)),
        (false, (true, false), st.normal, (false, false)),
        (false, (false, true), st.normal, (false, false)),
        (false, (true, true), st.normal, (false, true)),
        (false, (false, true), st.highlight, (true, true)),
        (false, (true, false), st.ghosted, (true, true)),
        (true, (true, false), st.ghosted, (false, true)),
        (true, (false, true), st.highlight, (true, false)),
        (false, (false, false), st.normal, (false, false)),
    ];
    for (s, (h, g), req, want) in cases {
        let btn = typed(0x1000_0001, vv.types.button, 0, 0, 10, 10);
        let btn = state(btn, st.normal, 0, vec![], vec![]);
        let mut ui = ui_one(LAYOUT, 100, 100, vec![btn], 100, 100);
        let k = open(&mut ui, 0x1000_0001);
        ui.set_property(k, pp.button_supports_ghosted_highlight, b(s));
        ui.set_property(k, pp.button_highlighted, b(h));
        ui.set_property(k, pp.button_ghosted, b(g));
        ui.switch_state(k, 0);
        ui.request_state(k, req);
        let got = (
            ui.prop(k, pp.button_highlighted).bool(),
            ui.prop(k, pp.button_ghosted).bool(),
        );
        assert_eq!(got, want, "S {s} start ({h}, {g}) request {req:#x}");
        if !s && (h, g) == (false, false) && req == st.normal {
            // Step 4: switched directly.
            assert_eq!(ui.state(k), Some(st.normal));
        }
    }
}

// ----- §15.8: GroupBox -------------------------------------------------------------------------

#[test]
fn example_15_8_groupbox() {
    let vv = v();
    let pp = vv.pids;
    let (a, bb, c) = (0x1000_00C1, 0x1000_00C2, 0x1000_00C3);
    let mut g = typed(0x1000_0001, vv.types.group_box, 0, 0, 300, 50);
    g = prop(g, pp.gb_default, en(bb));
    for (i, id) in [a, bb, c].into_iter().enumerate() {
        g = child(
            g,
            z(
                typed(id, vv.types.button, i as i32 * 60, 0, 50, 20),
                0,
                i as u32 + 1,
            ),
        );
    }
    let mut ui = ui_one(LAYOUT, 1024, 768, vec![g], 1024, 768);
    ui.register_handler(c, vv.msgs.button_click);
    let k = open(&mut ui, 0x1000_0001);
    let hl = |ui: &Ui, id| {
        ui.prop(ui.find_child(k, id).unwrap(), pp.button_highlighted)
            .bool()
    };
    assert!(hl(&ui, bb) && !hl(&ui, a) && !hl(&ui, c));
    let click_c = |ui: &mut Ui| {
        ui.pointer_moved(125, 5);
        ui.mouse_press(&cmd("SelectLeft"));
        ui.mouse_release(&cmd("SelectLeft"));
    };
    ui.take_events();
    click_c(&mut ui);
    assert!(hl(&ui, c) && !hl(&ui, bb));
    let game = |ev: &[UiEvent]| {
        ev.iter()
            .filter(
                |e| matches!(e, UiEvent::Game { message } if message.msg == vv.msgs.button_click),
            )
            .count()
    };
    assert_eq!(game(&ui.take_events()), 1);
    click_c(&mut ui);
    assert!(hl(&ui, c));
    assert_eq!(game(&ui.take_events()), 0, "the GroupBox stops the click");
}

// ----- §15.9: tree indentation ---------------------------------------------------------------

#[test]
fn example_15_9_tree_indentation() {
    let vv = v();
    let pp = vv.pids;
    let icon_id = 0x1000_0300;
    let mut tree = typed(0x1000_0001, vv.types.tree, 0, 0, 400, 400);
    tree = prop(tree, pp.tree_indent, int(8));
    tree = prop(tree, pp.tree_icon_layout, did(LAYOUT));
    tree = prop(tree, pp.tree_icon_id, en(icon_id));
    tree = prop(tree, pp.tree_auto_expand, b(true));
    tree = prop(tree, pp.lb_max_columns, int(1));
    let icon = field(icon_id, 0, 0, 16, 16);
    let n_entry = field(0x1000_0301, 0, 0, 100, 20);
    let m_entry = field(0x1000_0302, 0, 0, 90, 20);
    let mut ui = ui_one(
        LAYOUT,
        1024,
        768,
        vec![tree, icon, n_entry, m_entry],
        1024,
        768,
    );
    let t = open(&mut ui, 0x1000_0001);
    let n = ui
        .create_element(Some(t), Did(LAYOUT), 0x1000_0301)
        .unwrap();
    ui.list_add(t, n, None).unwrap();
    let m = ui
        .create_element(Some(t), Did(LAYOUT), 0x1000_0302)
        .unwrap();
    ui.list_add(t, m, Some(n)).unwrap();
    assert_eq!(ui.scroll_state(t).unwrap().0.0, 124);
    assert_eq!(ui.layout_box(n).unwrap().l, 16);
    assert_eq!(ui.layout_box(m).unwrap().l, 24);
    assert_eq!(ui.layout_box(m).unwrap().t, 20);
}

// ----- §15.10: options slider -----------------------------------------------------------------

#[test]
fn example_15_10_slider_track_and_jump() {
    let vv = v();
    let pp = vv.pids;
    let bt = vv.types.button;
    let bar = typed(BAR, vv.types.scrollbar, 0, 0, 322, 16);
    let bar = prop(bar, pp.sb_horizontal, b(true));
    let bar = prop(bar, pp.sb_slider, b(true));
    let bar = prop(bar, pp.sb_dec_button, en(DEC));
    let bar = prop(bar, pp.sb_inc_button, en(INC));
    let bar = prop(bar, pp.sb_position, f(0.5));
    let bar = child(bar, z(typed(vv.ids.widget, bt, 0, 0, 16, 16), 0, 1));
    let root = child(field(0x1000_0001, 0, 0, 500, 100), z(bar, 0, 1));
    // The arrow buttons are siblings of the bar in the template (§6.2).
    let root = child(root, z(typed(DEC, bt, 0, 40, 16, 16), 0, 2));
    let root = child(root, z(typed(INC, bt, 40, 40, 16, 16), 0, 3));
    let mut ui = ui_one(LAYOUT, 1024, 768, vec![root], 1024, 768);
    let r = open(&mut ui, 0x1000_0001);
    let bk = ui.find_child(r, BAR).unwrap();
    ui.frame_work();
    let dec = ui.find_child(bk, DEC).expect("adopted");
    let inc = ui.find_child(bk, INC).expect("adopted");
    assert_eq!(ui.layout_box(dec).unwrap().l, 0);
    assert_eq!(ui.layout_box(inc).unwrap().l, 306);
    let thumb = ui.find_child(bk, vv.ids.widget).unwrap();
    assert_eq!(ui.layout_box(thumb), Some(IBox::from_xywh(153, 0, 16, 16)));
    // Jump to the pointer at bar x = 100.
    ui.pointer_moved(100, 8);
    ui.mouse_press(&cmd("SelectLeft"));
    let q = ui.prop(bk, pp.sb_position).float();
    assert!((q - 76.0 / 273.0).abs() < 1e-6, "{q}");
    ui.mouse_release(&cmd("SelectLeft"));
}

// ----- §11: Menu -------------------------------------------------------------------------------

#[test]
fn menu_opens_below_and_closes() {
    let vv = v();
    let pp = vv.pids;
    let popup_id = 0x1000_0400;
    let lb_id = 0x1000_0401;
    let mut menu = typed(0x1000_0001, vv.types.menu, 100, 100, 80, 20);
    menu = prop(menu, pp.menu_popup, en(popup_id));
    menu = prop(menu, pp.menu_popup_layout, did(LAYOUT));
    menu = prop(menu, pp.menu_listbox, en(lb_id));
    let popup = child(
        field(popup_id, 0, 0, 120, 60),
        z(typed(lb_id, vv.types.list_box, 0, 0, 120, 60), 0, 1),
    );
    let root = child(field(0x1000_0002, 0, 0, 400, 400), z(menu, 0, 1));
    let mut ui = ui_one(
        LAYOUT,
        1024,
        768,
        vec![root, popup, field(ENTRY, 0, 0, 100, 20)],
        1024,
        768,
    );
    let r = open(&mut ui, 0x1000_0002);
    let mk = ui.find_child(r, 0x1000_0001).unwrap();
    let pk = ui.menu_popup(mk).unwrap();
    assert!(!ui.is_visible(pk));
    let lk = ui.find_child(pk, lb_id).unwrap();
    let e = ui.create_element(Some(lk), Did(LAYOUT), ENTRY).unwrap();
    ui.list_add(lk, e, None).unwrap();
    ui.pointer_moved(110, 105);
    ui.mouse_press(&cmd("SelectLeft"));
    ui.mouse_release(&cmd("SelectLeft"));
    assert!(ui.menu_is_open(mk));
    assert!(ui.is_visible(pk));
    let pb = ui.layout_box(pk).unwrap();
    assert_eq!((pb.l, pb.t), (100, 119));
    ui.pointer_moved(110, 105);
    ui.mouse_press(&cmd("SelectLeft"));
    ui.mouse_release(&cmd("SelectLeft"));
    assert!(!ui.menu_is_open(mk));
}

// ----- WI8, WI9 ---------------------------------------------------------------------------------

proptest! {
    /// WI8 and WI9 over random list contents and scroll requests.
    #[test]
    fn wi8_wi9_list(n in 0usize..40, horizontal in any::<bool>(), mc in -2i32..6, mr in -2i32..6, sx in -500i32..2000, sy in -500i32..2000) {
        let vv = v();
        let pp = vv.pids;
        let mut list = typed(LIST, vv.types.list_box, 0, 0, 200, 150);
        list = prop(list, pp.lb_horizontal, b(horizontal));
        // -2 = not set on the element.
        if mc != -2 {
            list = prop(list, pp.lb_max_columns, int(mc));
        }
        if mr != -2 {
            list = prop(list, pp.lb_max_rows, int(mr));
        }
        let root = child(field(0x1000_0001, 0, 0, 600, 600), z(list, 0, 1));
        let mut ui = ui_one(LAYOUT, 1024, 768, vec![root, field(ENTRY, 0, 0, 37, 23)], 1024, 768);
        let r = ui.open_panel(Did(LAYOUT), 0x1000_0001).unwrap();
        let l = ui.find_child(r, LIST).unwrap();
        for _ in 0..n {
            let e = ui.create_element(Some(l), Did(LAYOUT), ENTRY).unwrap();
            ui.list_add(l, e, None).unwrap();
        }
        let (cols, rows) = ui.list_grid(l).unwrap();
        let capped = (horizontal && mr != -2) || (!horizontal && mc != -2);
        if !capped || n == 0 {
            prop_assert!((cols as i64) * (rows as i64) >= n as i64);
        }
        let (content, _) = ui.scroll_state(l).unwrap();
        if cols > 0 && rows > 0 {
            prop_assert_eq!(content.0, 37 * cols);
            prop_assert_eq!(content.1, 23 * rows);
        } else {
            prop_assert_eq!(content, (0, 0));
        }
        ui.scroll_to(l, sx, sy, false);
        let (_, (ox, oy)) = ui.scroll_state(l).unwrap();
        prop_assert!(ox >= 0 && ox <= (content.0 - 200).max(0));
        prop_assert!(oy >= 0 && oy <= (content.1 - 150).max(0));
    }
}
