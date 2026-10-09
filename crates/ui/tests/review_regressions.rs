// SPDX-License-Identifier: GPL-3.0-or-later
//! Regression tests for review findings on the UI core: surfaces under hidden ancestors,
//! the nearest lookup, focus on press, text editing, scale, media, meters, nested
//! creation, tree nodes, destruction cost, drags and modal containers. All values are
//! invented.

mod ui_common;

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_formats::did::Did;
use tessera_formats::string_info::StringInfo;
use tessera_formats::ui_layout::{MediaItem, UiLayout};
use tessera_ui::geom::IBox;
use tessera_ui::paint::Unlimited;
use tessera_ui::{
    ControlKind, ElementKey, ImageRef, ImageState, MemoryEnv, PreparedText, TextRequest, Ui, UiEnv,
    UiEvent, UiFrame, Vocab,
};
use ui_common::*;

const LAYOUT: u32 = 0x22FF_7000;

fn open(ui: &mut Ui, id: u32) -> ElementKey {
    ui.open_panel(Did(LAYOUT), id).unwrap()
}

fn frames(ui: &mut Ui, n: u64) -> UiFrame {
    let mut budget = Unlimited;
    let mut last = UiFrame::default();
    for t in 0..n {
        last = ui.paint(Duration::from_millis(16 * t + 1), &mut budget);
    }
    last
}

// ----- surfaces under a hidden ancestor (decision 0015 Decision 1.1) ---------------------

#[test]
fn surface_under_a_hidden_ancestor_has_no_entry_and_keeps_its_canvas() {
    let vv = v();
    let pp = vv.pids;
    let g = prop(
        media(field(0x3, 10, 10, 16, 16), image_item(0x4100_0002, 2)),
        pp.object_mode,
        en(1),
    );
    let f = child(field(0x2, 0, 0, 50, 50), z(g, 0, 1));
    let root = child(
        media(field(0x1, 0, 0, 100, 100), image_item(0x4100_0001, 2)),
        z(f, 0, 1),
    );
    let mut env = env_with(vec![layout(LAYOUT, 1024, 768, vec![root])]);
    add_image(&mut env, 0x4100_0001, solid(100, 100, 0xFF00_0001));
    add_image(&mut env, 0x4100_0002, solid(16, 16, 0xFF00_0002));
    let mut ui = ui_with(env, 1024, 768);
    let k = open(&mut ui, 0x1);
    let fk = ui.find_child(k, 0x2).unwrap();
    let gk = ui.find_child(fk, 0x3).unwrap();
    let shown = frames(&mut ui, 4).composition.entries.len();
    assert!(shown >= 2, "root and nested entries");
    ui.set_shown(fk, false);
    assert!(!ui.is_displayed(gk));
    let fr = frames(&mut ui, 4);
    assert_eq!(fr.composition.entries.len(), 1, "only the root is composed");
    assert!(ui.surface_of(gk).is_some(), "the canvas is kept");
    ui.set_shown(fk, true);
    assert_eq!(frames(&mut ui, 4).composition.entries.len(), shown);
}

// ----- nearest lookup (UW §2.3) --------------------------------------------------------------

#[test]
fn nearest_lookup_ends_at_the_root_panel() {
    let vv = v();
    let pp = vv.pids;
    let bar = typed(0x10, vv.types.resize_bar, 190, 0, 10, 100);
    let bar = prop(bar, pp.resize_border_right, b(true));
    let bar = prop(bar, pp.resize_target, en(0x500));
    let a = child(
        child(field(0x1, 0, 0, 200, 100), z(bar, 0, 1)),
        z(field(0x11, 0, 0, 10, 10), 0, 2),
    );
    let bp = child(field(0x2, 400, 400, 100, 100), field(0x500, 0, 0, 40, 40));
    let mut ui = ui_one(LAYOUT, 1024, 768, vec![a, bp], 1024, 768);
    let ak = open(&mut ui, 0x1);
    let bk = open(&mut ui, 0x2);
    let bar = ui.find_child(ak, 0x10).unwrap();
    let sib = ui.find_child(ak, 0x11).unwrap();
    let other = ui.find_child(bk, 0x500).unwrap();
    assert_eq!(ui.find_nearest(bar, 0x11), Some(sib));
    assert_eq!(ui.find_nearest(bar, 0x500), None);
    // The resize falls back to the bar's parent; the other panel's 0x500 is untouched.
    let before = ui.layout_box(other);
    let a_before = ui.layout_box(ak).unwrap();
    ui.pointer_moved(195, 50);
    ui.mouse_press(&cmd("SelectLeft"));
    ui.pointer_moved(225, 50);
    ui.mouse_release(&cmd("SelectLeft"));
    assert_eq!(ui.layout_box(other), before);
    assert_ne!(ui.layout_box(ak).unwrap(), a_before);
}

// ----- focus on press and text editing (UB §5.2 OPEN, §5.4) -------------------------------

fn edit_ui() -> (Ui, ElementKey) {
    let vv = v();
    let pp = vv.pids;
    let t = prop(
        prop(
            typed(0x2, vv.types.text, 0, 0, 100, 20),
            pp.text_editable,
            b(true),
        ),
        pp.text_selectable,
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
    (ui, tk)
}

#[test]
fn pressing_an_editable_text_gives_it_focus() {
    let (mut ui, tk) = edit_ui();
    ui.pointer_moved(10, 10);
    assert_eq!(ui.hovered(), Some(tk));
    ui.mouse_press(&cmd("SelectLeft"));
    ui.mouse_release(&cmd("SelectLeft"));
    assert_eq!(ui.focus_element(), Some(tk));
    assert!(
        ui.band()
            .iter()
            .any(|e| e.map == "EditControls" && e.owner == tk && e.priority == 50)
    );
    assert!(ui.text_input('a'));
    assert_eq!(ui.text_of(tk), "a");
}

#[test]
fn editing_never_splits_a_surrogate_pair() {
    let (mut ui, tk) = edit_ui();
    ui.set_focus(Some(tk));
    ui.text_input('a');
    ui.text_input('\u{1F600}');
    assert!(ui.key_command(&cmd("BackspaceKey"), true, 1.0));
    assert_eq!(ui.text_of(tk), "a");
    ui.set_text(tk, Some(String::new()));
    ui.key_command(&cmd("CursorStartOfDocument"), true, 1.0);
    ui.text_input('\u{1F600}');
    ui.key_command(&cmd("CursorCharLeft"), true, 1.0);
    ui.text_input('x');
    assert_eq!(ui.text_of(tk), "x\u{1F600}");
    // Caret after "x": Delete removes the whole pair.
    ui.key_command(&cmd("DeleteKey"), true, 1.0);
    assert_eq!(ui.text_of(tk), "x");
    // A pair that would go one unit over max_chars is refused whole.
    ui.set_property(tk, v().pids.text_max_chars, int(2));
    ui.text_input('\u{1F600}');
    assert_eq!(ui.text_of(tk), "x");
    ui.text_input('y');
    assert_eq!(ui.text_of(tk), "xy");
}

#[test]
fn paste_asks_the_client_and_cut_copies_and_clears() {
    let (mut ui, tk) = edit_ui();
    ui.set_focus(Some(tk));
    ui.take_events();
    assert!(ui.key_command(&cmd("PasteText"), true, 1.0));
    assert!(
        ui.take_events()
            .contains(&UiEvent::PasteRequest { target: tk })
    );
    assert_eq!(ui.paste_text(tk, "hi\n!"), 3);
    assert_eq!(ui.text_of(tk), "hi!");
    assert!(ui.key_command(&cmd("CutText"), true, 1.0));
    assert!(ui.take_events().contains(&UiEvent::CopyText {
        text: "hi!".to_string()
    }));
    assert_eq!(ui.text_of(tk), "");
}

// ----- scale stored while hidden (UB §9.2 step 6) -----------------------------------------

#[test]
fn a_scale_stored_while_hidden_keeps_its_scaler_type() {
    let mk = || {
        let mut ui = ui_one(
            LAYOUT,
            1920,
            1080,
            vec![field(0x1, 100, 100, 200, 100)],
            1920,
            1080,
        );
        let k = open(&mut ui, 0x1);
        (ui, k)
    };
    let (mut shown, k1) = mk();
    shown.scale_panel(k1, 1.5, false);
    let (mut hidden, k2) = mk();
    hidden.set_shown(k2, false);
    hidden.scale_panel(k2, 1.5, false);
    hidden.set_shown(k2, true);
    assert_eq!(hidden.layout_box(k2), shown.layout_box(k1));
    assert_eq!(hidden.displayed_box(k2), shown.displayed_box(k1));
    assert_eq!(
        shown.layout_box(k1),
        Some(IBox::from_xywh(100, 100, 300, 150))
    );
}

// ----- media: a State item that causes no switch runs again (UL §9.5 step 2) ----------------

struct SeqEnv {
    inner: MemoryEnv,
    random: VecDeque<f32>,
}

impl UiEnv for SeqEnv {
    fn layout(&mut self, did: Did) -> Option<Arc<UiLayout>> {
        self.inner.layout(did)
    }
    fn image(&mut self, image: &ImageRef) -> ImageState {
        self.inner.image(image)
    }
    fn shape_text(&mut self, request: &TextRequest<'_>) -> Option<Arc<PreparedText>> {
        self.inner.shape_text(request)
    }
    fn resolve_string(&mut self, info: &StringInfo) -> Option<String> {
        self.inner.resolve_string(info)
    }
    fn random(&mut self) -> f32 {
        if self.random.len() > 1 {
            self.random.pop_front().unwrap_or(0.9)
        } else {
            self.random.front().copied().unwrap_or(0.9)
        }
    }
}

#[test]
fn state_item_without_a_switch_runs_again_next_frame() {
    let e = state(
        field(0x1, 0, 0, 16, 16),
        0x0B,
        0,
        vec![],
        vec![
            MediaItem::State {
                state: 0x0B,
                probability: 0.5,
            },
            image_item(0x4100_0001, 2),
        ],
    );
    let e = default_state(e, 0x0B);
    let env = SeqEnv {
        inner: env_with(vec![layout(LAYOUT, 1024, 768, vec![e])]),
        random: VecDeque::from([0.1, 0.9]),
    };
    let mut ui = Ui::new(Box::new(env), Vocab::default(), (1024, 768));
    let k = open(&mut ui, 0x1);
    assert_eq!(ui.base_image(k), None, "the first draw acts (no switch)");
    for i in 1..=4 {
        ui.set_time(f64::from(i) * 0.016);
        ui.frame_work();
    }
    assert_eq!(ui.base_image(k), Some((ImageRef::Did(0x4100_0001), 2)));
}

// ----- frame meter outside the array (UW §4.4) --------------------------------------------

#[test]
fn frame_meter_keeps_its_image_for_an_index_outside_the_array() {
    let vv = v();
    let pp = vv.pids;
    let frames = vec![did(0x4100_0301), did(0x4100_0302), did(0x4100_0303)];
    let m = typed(0x1000_0001, vv.types.meter, 0, 0, 20, 20);
    let m = prop(m, pp.meter_frame, b(true));
    let m = prop(m, pp.meter_frame_array, array(0x473, frames));
    let m = prop(m, pp.meter_position, f(0.5));
    let mut ui = ui_one(LAYOUT, 200, 200, vec![m], 200, 200);
    let k = open(&mut ui, 0x1000_0001);
    let d2 = Some((ImageRef::Did(0x4100_0302), 2));
    assert_eq!(ui.base_image(k), d2);
    ui.set_property(k, pp.meter_position, f(-1.0));
    assert_eq!(ui.base_image(k), d2, "index −1: no entry, image unchanged");
    ui.set_property(k, pp.meter_position, f(2.0));
    assert_eq!(ui.base_image(k), d2, "index 4: no entry, image unchanged");
    ui.set_property(k, pp.meter_position, f(0.0));
    assert_eq!(ui.base_image(k), Some((ImageRef::Did(0x4100_0301), 2)));
}

// ----- nested creation is bounded ------------------------------------------------------------

#[test]
fn a_menu_whose_popup_is_itself_does_not_recurse() {
    let vv = v();
    let pp = vv.pids;
    let mut menu = typed(0x1000_0001, vv.types.menu, 0, 0, 80, 20);
    menu = prop(menu, pp.menu_popup, en(0x1000_0001));
    menu = prop(menu, pp.menu_popup_layout, did(LAYOUT));
    let mut ui = ui_one(LAYOUT, 1024, 768, vec![menu], 1024, 768);
    let k = ui.open_panel(Did(LAYOUT), 0x1000_0001);
    assert!(k.is_ok());
    assert!(ui.census().creation_refused >= 1);
    assert!(ui.element_count() < 8, "{}", ui.element_count());
}

fn template(id: u32) -> tessera_formats::property::PropertyValue {
    let pp = v().pids;
    strukt(vec![
        (pp.lb_template_layout, did(LAYOUT)),
        (pp.lb_template_element, en(id)),
    ])
}

#[test]
fn a_list_whose_templates_name_itself_does_not_recurse() {
    let vv = v();
    let pp = vv.pids;
    let mut list = typed(0x1000_0001, vv.types.list_box, 0, 0, 200, 200);
    list = prop(list, pp.lb_autofill, b(true));
    list = prop(
        list,
        pp.lb_templates,
        array(0x51D, vec![template(0x1000_0001), template(0x1000_0001)]),
    );
    let mut ui = ui_one(LAYOUT, 1024, 768, vec![list], 1024, 768);
    let t0 = Instant::now();
    let k = ui.open_panel(Did(LAYOUT), 0x1000_0001).unwrap();
    assert!(t0.elapsed() < Duration::from_secs(10));
    assert!(ui.list_entries(k).is_empty());
    assert!(ui.census().creation_refused >= 2);
    assert_eq!(ui.element_count(), 2, "the screen root and the list");
}

#[test]
fn template_chains_stop_at_the_nesting_cap() {
    let vv = v();
    let pp = vv.pids;
    let n = tessera_ui::MAX_CREATE_NESTING as u32 + 8;
    let elements: Vec<_> = (0..n)
        .map(|i| {
            let mut l = typed(0x1000_0001 + i, vv.types.list_box, 0, 0, 50, 50);
            l = prop(l, pp.lb_autofill, b(true));
            prop(
                l,
                pp.lb_templates,
                array(0x51D, vec![template(0x1000_0002 + i)]),
            )
        })
        .collect();
    let mut ui = ui_one(LAYOUT, 1024, 768, elements, 1024, 768);
    ui.open_panel(Did(LAYOUT), 0x1000_0001).unwrap();
    assert!(ui.census().creation_refused >= 1);
    assert_eq!(
        ui.element_count(),
        1 + tessera_ui::MAX_CREATE_NESTING,
        "the screen root and one list per nesting level"
    );
}

// ----- tree nodes ------------------------------------------------------------------------------

fn tree_ui(with_icon: bool) -> (Ui, ElementKey) {
    let vv = v();
    let pp = vv.pids;
    let icon_id = 0x1000_0300;
    let mut tree = typed(0x1000_0001, vv.types.tree, 0, 0, 400, 400);
    tree = prop(tree, pp.tree_indent, int(8));
    if with_icon {
        tree = prop(tree, pp.tree_icon_layout, did(LAYOUT));
        tree = prop(tree, pp.tree_icon_id, en(icon_id));
    }
    tree = prop(tree, pp.tree_auto_expand, b(true));
    tree = prop(tree, pp.lb_max_columns, int(1));
    let mut ui = ui_one(
        LAYOUT,
        1024,
        768,
        vec![
            tree,
            field(icon_id, 0, 0, 16, 16),
            field(0x1000_0301, 0, 0, 100, 20),
            field(0x1000_0302, 0, 0, 90, 20),
        ],
        1024,
        768,
    );
    let t = open(&mut ui, 0x1000_0001);
    (ui, t)
}

fn entry(ui: &mut Ui, t: ElementKey, id: u32) -> ElementKey {
    ui.create_element(Some(t), Did(LAYOUT), id).unwrap()
}

#[test]
fn removing_a_node_keeps_later_parent_links() {
    let (mut ui, t) = tree_ui(false);
    let a = entry(&mut ui, t, 0x1000_0301);
    ui.list_add(t, a, None).unwrap();
    let bk = entry(&mut ui, t, 0x1000_0301);
    ui.list_add(t, bk, None).unwrap();
    let c = entry(&mut ui, t, 0x1000_0302);
    ui.list_add(t, c, Some(bk)).unwrap();
    ui.list_remove(t, a);
    // B: 1 × 8 + 100; C: 2 × 8 + 90.
    assert_eq!(ui.scroll_state(t).unwrap().0.0, 108);
    assert_eq!(ui.layout_box(c).unwrap().l, 8);
    // Collapse and expand still act on the right nodes.
    ui.tree_set_expanded(t, bk, false);
    assert!(!ui.is_visible(c));
    ui.tree_set_expanded(t, c, true);
    assert!(ui.is_visible(c), "expanding C expands B first");
}

#[test]
fn removed_nodes_take_their_icon_and_children_move_up() {
    let (mut ui, t) = tree_ui(true);
    let base = ui.element_count();
    for _ in 0..100 {
        let n = entry(&mut ui, t, 0x1000_0301);
        ui.list_add(t, n, None).unwrap();
        ui.list_remove(t, n);
    }
    assert_eq!(ui.element_count(), base);
    assert!(ui.children(t).is_empty());
    // A second add of the same entry makes no second node or icon.
    let n = entry(&mut ui, t, 0x1000_0301);
    ui.list_add(t, n, None).unwrap();
    ui.list_add(t, n, None).unwrap();
    assert_eq!(ui.children(t).len(), 2);
    // A child of a removed node becomes a top-level node, shown.
    let c = entry(&mut ui, t, 0x1000_0302);
    ui.list_add(t, c, Some(n)).unwrap();
    ui.tree_set_expanded(t, n, false);
    assert!(!ui.is_visible(c));
    ui.list_remove(t, n);
    assert!(ui.is_visible(c));
    assert_eq!(ui.list_entries(t), vec![c]);
    assert_eq!(ui.children(t).len(), 2, "C and its icon");
}

#[test]
fn a_plugin_tree_survives_node_removal_and_reparenting() {
    let mut ui = ui_with(MemoryEnv::default(), 1920, 1080);
    let w = ui.create_control(ControlKind::Window, 1).unwrap();
    ui.control_set_size(w, 400, 400);
    ui.control_set_visible(w, true);
    let t = ui.create_control(ControlKind::TreeView, 1).unwrap();
    ui.control_set_parent(t, Some(w)).unwrap();
    ui.control_set_visible(t, true);
    let mk = |ui: &mut Ui| {
        let c = ui.create_control(ControlKind::Control, 1).unwrap();
        ui.control_set_visible(c, true);
        c
    };
    let r = mk(&mut ui);
    let p = mk(&mut ui);
    let x = mk(&mut ui);
    ui.list_add(t, r, None).unwrap();
    ui.list_add(t, p, None).unwrap();
    ui.list_add(t, x, Some(p)).unwrap();
    ui.list_remove(t, r);
    let t0 = Instant::now();
    ui.tree_set_expanded(t, x, true);
    assert!(t0.elapsed() < Duration::from_secs(2));
    ui.tree_set_expanded(t, p, false);
    assert!(!ui.is_visible(x));
    ui.tree_set_expanded(t, p, true);
    assert!(ui.is_visible(x));
    // Moving X out of the tree drops its node: collapsing P no longer hides it.
    ui.control_set_parent(x, Some(w)).unwrap();
    assert!(!ui.list_entries(t).contains(&x));
    ui.tree_set_expanded(t, p, false);
    assert!(ui.is_visible(x));
}

// ----- destruction cost -------------------------------------------------------------------------

#[test]
fn destroying_many_controls_is_linear() {
    let mut ui = ui_with(MemoryEnv::default(), 1920, 1080);
    let w = ui.create_control(ControlKind::Window, 7).unwrap();
    for _ in 0..20_000 {
        let c = ui.create_control(ControlKind::Control, 7).unwrap();
        ui.control_set_parent(c, Some(w)).unwrap();
    }
    assert_eq!(ui.element_count(), 20_002);
    let t0 = Instant::now();
    ui.destroy_apartment(7);
    let took = t0.elapsed();
    assert_eq!(ui.element_count(), 1);
    // Indicative (decision 0010); the quadratic version took over 100 s in a debug build.
    assert!(took < Duration::from_secs(10), "{took:?}");
}

// ----- drag position saturates ---------------------------------------------------------------

#[test]
fn a_drag_from_a_far_moved_control_does_not_overflow() {
    let mut ui = ui_with(MemoryEnv::default(), 1920, 1080);
    let w = ui.create_control(ControlKind::Window, 1).unwrap();
    ui.control_set_size(w, 100, 100);
    ui.control_set_visible(w, true);
    ui.control_set_stretch_mode(w, 1);
    let c = ui.create_control(ControlKind::Control, 1).unwrap();
    ui.control_set_parent(c, Some(w)).unwrap();
    ui.control_set_visible(c, true);
    ui.control_set_size(w, i32::MAX, i32::MAX);
    ui.pointer_moved(10, 10);
    ui.mouse_press(&cmd("SelectLeft"));
    ui.control_set_position(c, -(1 << 24), 0);
    ui.pointer_moved(20, 10);
    ui.pointer_moved(40, 10);
    ui.mouse_release(&cmd("SelectLeft"));
}

// ----- modal containers and saved registrations go with their element -----------------------

#[test]
fn a_modal_dialog_takes_its_container_with_it() {
    let vv = v();
    let pp = vv.pids;
    let d = prop(field(0x1, 0, 0, 100, 50), pp.dialog_modal, b(true));
    let mut ui = ui_one(LAYOUT, 1024, 768, vec![d], 1024, 768);
    for _ in 0..2 {
        let k = open(&mut ui, 0x1);
        assert_eq!(ui.element_count(), 3, "screen root, container, dialog");
        ui.dialog_close(k, true);
        ui.destroy(k);
        assert_eq!(ui.element_count(), 1);
    }
}

#[test]
fn a_destroyed_element_is_no_longer_registered_as_saved() {
    let vv = v();
    let pp = vv.pids;
    let d = prop(field(0x1, 0, 0, 100, 50), pp.save_loc, b(true));
    let mut ui = ui_one(LAYOUT, 1024, 768, vec![d], 1024, 768);
    let k = open(&mut ui, 0x1);
    assert!(ui.is_saved_element(k));
    ui.destroy(k);
    assert!(!ui.is_saved_element(k));
}
