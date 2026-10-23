// SPDX-License-Identifier: GPL-3.0-or-later
//! Engine dialog boxes (`ui-widgets.md` §18) and context menus (§19) on synthetic layouts
//! `Dialog` and `ContextMenu` (invented element trees with the widgets' placeholder IDs and
//! the boxes §18.1 and §19.1 give, generated art, the bundled font), and the code-made
//! fallbacks without a layout. Two committed tier-1 goldens (decision 0031): a
//! confirmation dialog and a nested menu (`tests/golden/widgets_<name>.png`;
//! `TESSERA_BLESS=1` rewrites them). No install file is opened; all values are invented.

mod p5_common;

use std::sync::Arc;

use p5_common::ui_common::{
    b, child, edges, field, image_item, int, layout, media, prop, state, typed,
};
use p5_common::{ART, Rgba, TestEnv, button, capture, golden_file, real_click, settle};
use tessera_formats::did::Did;
use tessera_formats::ui_layout::ElementDesc;
use tessera_ui::game::chat::notice::{NoTables, TextRef, TextSource};
use tessera_ui::game::context_menu::{
    ContextMenu, ItemKind, MenuArt, MenuEvent, MenuItem, MenuPosition, MenuRefs, MenuReply,
    MenuRequest, TEXT_PAD,
};
use tessera_ui::game::dialog::{
    COUNTDOWN_VARIABLE, DialogBoxes, DialogKind, DialogOutcome, DialogPids, DialogRefs,
    DialogRequest, DialogText, MAX_QUEUED, engine_dialog_shown,
};
use tessera_ui::{Command, ControlKind, ElementKey, ImageRef, Ui, UiEvent, Vocab};

/// The synthetic `ContextMenu` layout.
const MENU_LAYOUT: u32 = 0x22FF_7000;
/// The synthetic `Dialog` layout.
const DIALOG_LAYOUT: u32 = 0x22FF_7100;
/// A synthetic panel to click outside the menus.
const PANEL_LAYOUT: u32 = 0x22FF_7200;
const PANEL: u32 = 0x7D10_0001;

fn vv() -> Vocab {
    Vocab::default()
}

fn art(i: u32) -> ImageRef {
    ImageRef::Did(ART + i)
}

fn menu_refs() -> MenuRefs {
    MenuRefs {
        layout: Some(Did(MENU_LAYOUT)),
        ..MenuRefs::fallback(MenuArt::default())
    }
}

fn fallback_menu_refs() -> MenuRefs {
    MenuRefs::fallback(MenuArt {
        background: Some(art(2)),
        rollover: Some(art(4)),
        check: Some(art(19)),
        unchecked: Some(art(11)),
        arrow: Some(art(17)),
    })
}

fn dialog_refs() -> DialogRefs {
    DialogRefs {
        layout: Some(Did(DIALOG_LAYOUT)),
        ..DialogRefs::default()
    }
}

fn text_el(id: u32, x: i32, y: i32, w: i32, h: i32) -> ElementDesc {
    typed(id, vv().types.text, x, y, w, h)
}

fn button_el(id: u32, x: i32, y: i32, w: i32, h: i32) -> ElementDesc {
    media(
        typed(id, vv().types.button, x, y, w, h),
        image_item(ART + 5, 4),
    )
}

/// The synthetic `ContextMenu` layout (§19.1 boxes).
fn context_menu_layout() -> tessera_formats::ui_layout::UiLayout {
    let r = menu_refs();
    let ids = r.ids;
    let st = vv().states;
    let lb = vv().types.list_box;
    let bg = || {
        edges(
            media(field(0x7D20_0001, 0, 0, 30, 30), image_item(ART + 2, 4)),
            [0, 0, 5, 5],
        )
    };
    let element = child(
        child(
            prop(
                field(ids.element, 0, 0, 30, 30),
                r.pids.open_delay,
                tessera_formats::property::PropertyValue::Float(0.5),
            ),
            bg(),
        ),
        typed(ids.list, lb, 2, 2, 26, 26),
    );
    let titled = child(
        child(
            child(field(ids.with_title, 0, 0, 30, 30), bg()),
            child(
                media(field(ids.title, 0, 0, 30, 16), image_item(ART + 1, 4)),
                text_el(ids.title_text, 4, 2, 22, 12),
            ),
        ),
        typed(ids.list, lb, 2, 16, 28, 14),
    );
    let map_menu = child(
        child(field(ids.map_menu, 0, 0, 30, 30), bg()),
        typed(ids.list, lb, 2, 2, 26, 26),
    );
    let item = |id: u32, w: i32, h: i32| {
        let mut d = field(id, 0, 0, w, h);
        d = state(d, st.normal, 0, vec![], vec![]);
        d = state(
            d,
            st.normal_rollover,
            0,
            vec![],
            vec![image_item(ART + 4, 4)],
        );
        d = state(d, st.ghosted, 0, vec![], vec![image_item(ART + 23, 4)]);
        d = child(d, field(ids.icon, 4, 0, 16, 16));
        child(d, text_el(ids.text, 22, 2, 6, 12))
    };
    let check = child(
        item(ids.check_item, 44, 18),
        state(
            media(field(ids.flag, 26, 0, 16, 16), image_item(ART + 19, 4)),
            st.ghosted,
            0,
            vec![],
            vec![image_item(ART + 11, 4)],
        ),
    );
    let sub = child(
        item(ids.sub_item, 44, 18),
        media(
            field(ids.open_arrow, 26, 0, 16, 16),
            image_item(ART + 17, 4),
        ),
    );
    layout(
        MENU_LAYOUT,
        1024,
        768,
        vec![
            element,
            titled,
            map_menu,
            item(ids.entry, 28, 16),
            check,
            sub,
        ],
    )
}

/// The synthetic `Dialog` layout (§18.1 boxes).
fn dialog_layout() -> tessera_formats::ui_layout::UiLayout {
    let ids = dialog_refs().ids;
    let p = vv().pids;
    let frame = |id: u32, w: i32, h: i32| media(field(id, 256, 280, w, h), image_item(ART, 4));
    // §18.1: the question is centred both ways.
    let dp = DialogPids::default();
    let centre = tessera_formats::property::PropertyValue::Enum(tessera_ui::text::JUSTIFY_CENTER);
    let question = prop(
        prop(
            text_el(ids.question, 6, 7, 500, 142),
            dp.text_horizontal,
            centre.clone(),
        ),
        dp.text_vertical,
        centre,
    );
    let confirmation = child(
        child(
            child(frame(ids.confirmation_field, 512, 183), question),
            button_el(ids.yes, 144, 156, 106, 20),
        ),
        button_el(ids.no, 263, 156, 106, 20),
    );
    let message = child(
        child(
            frame(ids.message_field, 512, 200),
            text_el(ids.statement, 10, 22, 493, 121),
        ),
        button_el(ids.ok, 192, 170, 96, 20),
    );
    let entry = prop(
        prop(text_el(ids.entry, 2, 2, 488, 20), p.text_editable, b(true)),
        p.text_max_chars,
        int(100),
    );
    let input = child(
        child(
            child(
                child(
                    frame(ids.text_input_field, 512, 180),
                    text_el(ids.input_question, 13, 25, 489, 68),
                ),
                child(
                    media(field(0x7D30_0001, 10, 96, 492, 24), image_item(ART + 10, 4)),
                    entry,
                ),
            ),
            button_el(ids.done, 184, 151, 64, 20),
        ),
        button_el(ids.quit, 264, 151, 64, 20),
    );
    let wait = child(
        frame(ids.wait_field, 512, 200),
        text_el(ids.wait_text, 10, 22, 493, 121),
    );
    layout(
        DIALOG_LAYOUT,
        1024,
        768,
        vec![confirmation, message, input, wait],
    )
}

fn panel_layout() -> tessera_formats::ui_layout::UiLayout {
    let p = vv().pids;
    layout(
        PANEL_LAYOUT,
        1024,
        768,
        vec![prop(
            media(field(PANEL, 600, 500, 200, 100), image_item(ART + 2, 4)),
            p.activatable,
            b(true),
        )],
    )
}

fn env() -> TestEnv {
    let mut env = TestEnv::new();
    for l in [context_menu_layout(), dialog_layout(), panel_layout()] {
        env.layouts.insert(l.did.0, Arc::new(l));
    }
    let dp = DialogPids::default();
    env.justify = Some((dp.text_horizontal, dp.text_vertical));
    env
}

fn ui(w: i32, h: i32) -> Ui {
    Ui::new(Box::new(env()), Vocab::default(), (w, h))
}

/// A game-code message from `k`.
fn msg(ui: &Ui, k: ElementKey, m: u32, value: i64, command: Option<&str>) -> UiEvent {
    UiEvent::Game {
        message: tessera_ui::Message {
            seq: 0,
            source: k,
            source_id: ui.element_id(k).unwrap_or(0),
            msg: m,
            arg: tessera_ui::MsgArg {
                value,
                command: command.map(Command::named),
                ..tessera_ui::MsgArg::default()
            },
        },
    }
}

fn left_click(ui: &Ui, k: ElementKey) -> UiEvent {
    msg(ui, k, ui.vocab().msgs.mouse_click, 0, Some("SelectLeft"))
}

fn hover(ui: &Ui, k: ElementKey, on: bool) -> UiEvent {
    msg(ui, k, ui.vocab().msgs.mouse_over_top, i64::from(on), None)
}

fn sample_items() -> Vec<MenuItem> {
    vec![
        MenuItem::plain(1, "Show in log").with_value(11),
        MenuItem::plain(2, "Remove everything here"),
        MenuItem::plain(3, "Ghosted").enabled(false),
    ]
}

/// The text width the test font gives `s`.
fn text_w(ui: &mut Ui, el: ElementKey) -> i32 {
    ui.measure_text_width(el).unwrap_or(0)
}

// ----- context menus ------------------------------------------------------------------

#[test]
fn items_take_the_widest_items_width_and_the_list_its_height() {
    let mut ui = ui(1024, 768);
    ui.pointer_moved(100, 120);
    let r = menu_refs();
    let m = ContextMenu::open(&mut ui, &r, &MenuRequest::new(sample_items())).expect("menu");
    let root = m.root().expect("root");
    assert_eq!(ui.element_id(root), Some(r.ids.element));
    let items = m.item_elements(0);
    assert_eq!(items.len(), 3);
    // Item width = text width + 10 + (28 − 6); the widest wins for all.
    let texts: Vec<ElementKey> = items
        .iter()
        .map(|k| tessera_ui::game::find_below(&ui, *k, r.ids.text).expect("text"))
        .collect();
    let widest = texts
        .iter()
        .map(|t| {
            let t = *t;
            text_w(&mut ui, t)
        })
        .max()
        .unwrap();
    let w = widest + TEXT_PAD + (28 - 6);
    let mut sum = 0;
    for k in &items {
        let bx = ui.layout_box(*k).unwrap();
        assert_eq!(bx.w(), w);
        sum += bx.h();
    }
    let list = ui.find_descendant(root, r.ids.list).unwrap();
    let lb = ui.layout_box(list).unwrap();
    assert_eq!((lb.w(), lb.h()), (w, sum));
    let mb = ui.layout_box(root).unwrap();
    assert_eq!(
        (mb.w(), mb.h()),
        (w + 4, sum + 4),
        "list plus the 2-pixel margins"
    );
    // The top-left corner is at the pointer; the disabled item is ghosted.
    let d = ui.displayed_box(root);
    assert_eq!((d.l, d.t), (100, 120));
    assert_eq!(ui.state(items[2]), Some(ui.vocab().states.ghosted));
    assert_eq!(ui.active_panel(), Some(root), "the menu is active");
}

#[test]
fn a_title_chooses_the_titled_element_and_widens_the_menu() {
    let mut ui = ui(1024, 768);
    let r = menu_refs();
    let mut req = MenuRequest::new(vec![MenuItem::plain(1, "x")]);
    req.title = "A rather long menu heading".into();
    let m = ContextMenu::open(&mut ui, &r, &req).expect("menu");
    let root = m.root().unwrap();
    assert_eq!(ui.element_id(root), Some(r.ids.with_title));
    let tt = ui.find_descendant(root, r.ids.title_text).unwrap();
    assert_eq!(ui.text_of(tt), "A rather long menu heading");
    let title_w = text_w(&mut ui, tt) + (30 - 22);
    let item = m.item_elements(0)[0];
    assert_eq!(
        ui.layout_box(item).unwrap().w(),
        title_w,
        "at least the title's width"
    );
}

#[test]
fn an_item_of_an_invalid_kind_shows_no_menu() {
    let mut ui = ui(1024, 768);
    let before = ui.children(ui.screen_root()).len();
    let mut bad = MenuItem::plain(9, "bad");
    bad.kind = ItemKind::from_raw(7);
    let r = ContextMenu::open(
        &mut ui,
        &menu_refs(),
        &MenuRequest::new(vec![MenuItem::plain(1, "ok"), bad]),
    );
    assert_eq!(
        r.err(),
        Some(tessera_ui::game::GameError::InvalidMenuKind(7)),
        "the raw kind is reported"
    );
    assert_eq!(
        ui.children(ui.screen_root()).len(),
        before,
        "nothing else happens"
    );
    // A submenu with an invalid item is not shown when opened; the root is.
    let mut bad = MenuItem::plain(9, "bad");
    bad.kind = ItemKind::Other(0);
    let mut m = ContextMenu::open(
        &mut ui,
        &menu_refs(),
        &MenuRequest::new(vec![MenuItem::submenu(1, "sub", vec![bad])]),
    )
    .expect("root shown");
    let item = m.item_elements(0)[0];
    let ev = left_click(&ui, item);
    assert_eq!(m.handle_event(&mut ui, &ev), MenuReply::Consumed);
    assert_eq!(m.menus().len(), 1, "the submenu is not shown");
}

#[test]
fn a_long_list_is_capped_at_four_fifths_of_the_screen_and_a_wide_menu_is_scaled() {
    let mut ui = ui(400, 300);
    let items: Vec<MenuItem> = (0..40).map(|i| MenuItem::plain(i, "Item")).collect();
    let r = menu_refs();
    let m = ContextMenu::open(&mut ui, &r, &MenuRequest::new(items)).expect("menu");
    let root = m.root().unwrap();
    let list = ui.find_descendant(root, r.ids.list).unwrap();
    assert_eq!(ui.layout_box(list).unwrap().h(), 240, "0.8 × 300");
    let d = ui.displayed_box(root);
    assert!(d.b <= 300 && d.t >= 0, "moved onto the screen: {d:?}");
    drop(m);
    // A label wider than the screen: scaled down uniformly to fit.
    let long = "W".repeat(120);
    let m = ContextMenu::open(
        &mut ui,
        &r,
        &MenuRequest::new(vec![MenuItem::plain(1, &long)]),
    )
    .expect("menu");
    let root = m.root().unwrap();
    let (sx, sy, _) = ui.node_scale_at_or_above(root);
    assert!(
        sx < 1.0 && (sx - sy).abs() < 1e-6,
        "uniform scale {sx} {sy}"
    );
    let d = ui.displayed_box(root);
    assert!(d.w() <= 400 && d.l >= 0 && d.r <= 400, "{d:?}");
}

#[test]
fn positions_and_submenu_placement() {
    let mut ui = ui(1024, 768);
    let r = menu_refs();
    let mut req = MenuRequest::new(vec![MenuItem::submenu(
        1,
        "Regions",
        vec![MenuItem::plain(10, "North"), MenuItem::plain(11, "South")],
    )]);
    req.position = Some(MenuPosition::BottomLeft(50, 400));
    let mut m = ContextMenu::open(&mut ui, &r, &req).expect("menu");
    let root = m.root().unwrap();
    let d = ui.displayed_box(root);
    assert_eq!((d.l, d.t + d.h()), (50, 400), "the bottom-left corner");
    let item = m.item_elements(0)[0];
    let ev = left_click(&ui, item);
    assert_eq!(m.handle_event(&mut ui, &ev), MenuReply::Consumed);
    let menus = m.menus();
    assert_eq!(menus.len(), 2, "the submenu opened");
    let ib = ui.displayed_box(item);
    let sb = ui.displayed_box(menus[1]);
    assert_eq!(
        (sb.l, sb.t),
        (ib.l + ib.w(), ib.t),
        "at the item's right edge, level with it"
    );
    assert_eq!(
        ui.prop(menus[1], r.pids.level).int(),
        1,
        "UICore_ContextMenu_lvl of the submenu"
    );
    // A second click on the submenu item closes the submenu.
    let ev = left_click(&ui, item);
    m.handle_event(&mut ui, &ev);
    assert_eq!(m.menus().len(), 1);
    m.close(&mut ui);
    // Near the right edge the submenu opens to the left of its item.
    let mut req = req.clone();
    req.position = Some(MenuPosition::TopLeft(1000, 10));
    let mut m = ContextMenu::open(&mut ui, &r, &req).expect("menu");
    let item = m.item_elements(0)[0];
    let ev = left_click(&ui, item);
    m.handle_event(&mut ui, &ev);
    let menus = m.menus();
    let ib = ui.displayed_box(item);
    let sb = ui.displayed_box(menus[1]);
    assert_eq!(sb.l + sb.w(), ib.l, "left of the item");
    let rb = ui.displayed_box(menus[0]);
    assert!(rb.l + rb.w() <= 1024, "the root moved onto the screen");
}

#[test]
fn hover_rolls_over_and_the_open_delay_opens_a_submenu() {
    let mut ui = ui(1024, 768);
    let r = menu_refs();
    let st = ui.vocab().states;
    let req = MenuRequest::new(vec![
        MenuItem::plain(1, "Plain"),
        MenuItem::submenu(2, "More", vec![MenuItem::plain(20, "Deeper")]),
    ]);
    let mut m = ContextMenu::open(&mut ui, &r, &req).expect("menu");
    let items = m.item_elements(0);
    ui.set_time(10.0);
    let ev = hover(&ui, items[1], true);
    assert_eq!(m.handle_event(&mut ui, &ev), MenuReply::Consumed);
    assert_eq!(ui.state(items[1]), Some(st.normal_rollover));
    ui.set_time(10.4);
    m.update(&mut ui);
    assert_eq!(m.menus().len(), 1, "not before the delay");
    ui.set_time(10.6);
    m.update(&mut ui);
    assert_eq!(m.menus().len(), 2, "opened after resting 0.5 s");
    // Resting on another item closes the open submenu.
    let ev = hover(&ui, items[1], false);
    m.handle_event(&mut ui, &ev);
    assert_eq!(ui.state(items[1]), Some(st.normal));
    let ev = hover(&ui, items[0], true);
    m.handle_event(&mut ui, &ev);
    ui.set_time(11.2);
    m.update(&mut ui);
    assert_eq!(m.menus().len(), 1);
}

#[test]
fn clicks_select_flip_marks_report_closes_and_ignore_disabled_items() {
    let mut ui = ui(1024, 768);
    let r = menu_refs();
    let st = ui.vocab().states;
    // A plain item: selected with its ID, value and path; the tree closes.
    let mut m = ContextMenu::open(&mut ui, &r, &MenuRequest::new(sample_items())).unwrap();
    let items = m.item_elements(0);
    let root = m.root().unwrap();
    let ev = left_click(&ui, items[2]);
    assert_eq!(
        m.handle_event(&mut ui, &ev),
        MenuReply::Consumed,
        "disabled"
    );
    assert!(m.is_open());
    let ev = left_click(&ui, items[0]);
    assert_eq!(
        m.handle_event(&mut ui, &ev),
        MenuReply::Event(MenuEvent::Selected {
            id: 1,
            value: 11,
            path: vec![0],
            checked: None
        })
    );
    assert!(!m.is_open() && !ui.exists(root));
    // A check item flips its mark; with keep_open the tree stays.
    let mut req = MenuRequest::new(vec![
        MenuItem::check(5, "Vendors", true),
        MenuItem::plain(6, "Close me").reporting_close(),
    ]);
    req.keep_open = true;
    let mut m = ContextMenu::open(&mut ui, &r, &req).unwrap();
    let items = m.item_elements(0);
    let mark = m.mark_element(0, 0).expect("mark");
    assert!(ui.is_visible(mark));
    assert_eq!(
        ui.state(mark),
        Some(0),
        "normal_activated is undefined: own block"
    );
    let ev = left_click(&ui, items[0]);
    let reply = m.handle_event(&mut ui, &ev);
    assert_eq!(
        reply,
        MenuReply::Event(MenuEvent::Selected {
            id: 5,
            value: 0,
            path: vec![0],
            checked: Some(false)
        })
    );
    assert!(m.is_open());
    assert!(!ui.is_visible(mark));
    assert_eq!(ui.state(mark), Some(st.ghosted));
    let ev = left_click(&ui, items[1]);
    assert_eq!(
        m.handle_event(&mut ui, &ev),
        MenuReply::Event(MenuEvent::CloseItem { id: 6, value: 0 })
    );
    assert!(!m.is_open());
}

#[test]
fn an_activation_outside_the_tree_closes_it_and_inside_does_not() {
    let mut ui = ui(1024, 768);
    let panel = ui.open_panel(Did(PANEL_LAYOUT), PANEL).expect("panel");
    let r = menu_refs();
    let req = MenuRequest {
        position: Some(MenuPosition::TopLeft(100, 100)),
        ..MenuRequest::new(vec![
            MenuItem::submenu(1, "Sub", vec![MenuItem::plain(10, "Leaf")]),
            MenuItem::plain(2, "Other"),
        ])
    };
    let mut m = ContextMenu::open(&mut ui, &r, &req).unwrap();
    let item = m.item_elements(0)[0];
    // Open the submenu by a real click, then click inside it: no dismissal.
    for ev in real_click(&mut ui, item) {
        assert_ne!(
            m.handle_event(&mut ui, &ev),
            MenuReply::Event(MenuEvent::Dismissed)
        );
    }
    assert_eq!(m.menus().len(), 2);
    let leaf = m.item_elements(1)[0];
    let mut got = None;
    for ev in real_click(&mut ui, leaf) {
        if let MenuReply::Event(e) = m.handle_event(&mut ui, &ev) {
            got = Some(e);
        }
    }
    assert_eq!(
        got,
        Some(MenuEvent::Selected {
            id: 10,
            value: 0,
            path: vec![0, 0],
            checked: None
        })
    );
    // Again, then a click on the other panel dismisses the tree.
    let mut m = ContextMenu::open(&mut ui, &r, &req).unwrap();
    let mut got = None;
    for ev in real_click(&mut ui, panel) {
        if let MenuReply::Event(e) = m.handle_event(&mut ui, &ev) {
            got = Some(e);
        }
    }
    assert_eq!(got, Some(MenuEvent::Dismissed));
    assert!(!m.is_open());
}

#[test]
fn the_fallback_menu_follows_the_same_rules() {
    let mut ui = ui(1024, 768);
    let r = fallback_menu_refs();
    let req = MenuRequest {
        title: "Heading".into(),
        ..MenuRequest::new(vec![
            MenuItem::check(1, "Mark", true),
            MenuItem::submenu(2, "Sub", vec![MenuItem::plain(3, "Leaf")]),
        ])
    };
    let mut m = ContextMenu::open(&mut ui, &r, &req).unwrap();
    let items = m.item_elements(0);
    let w = ui.layout_box(items[0]).unwrap().w();
    assert_eq!(ui.layout_box(items[1]).unwrap().w(), w, "common width");
    let ev = left_click(&ui, items[1]);
    m.handle_event(&mut ui, &ev);
    assert_eq!(m.menus().len(), 2);
    let leaf = m.item_elements(1)[0];
    let ev = left_click(&ui, leaf);
    assert!(matches!(
        m.handle_event(&mut ui, &ev),
        MenuReply::Event(MenuEvent::Selected { id: 3, .. })
    ));
    assert!(!m.is_open());
}

/// A submenu that is the active panel closes by hover: its parent becomes active, so a
/// later press outside still closes the tree (§19.4 step 4).
#[test]
fn closing_the_active_submenu_keeps_the_tree_closable() {
    let mut ui = ui(1024, 768);
    let panel = ui.open_panel(Did(PANEL_LAYOUT), PANEL).expect("panel");
    let r = menu_refs();
    let req = MenuRequest {
        position: Some(MenuPosition::TopLeft(100, 100)),
        ..MenuRequest::new(vec![
            MenuItem::submenu(1, "Sub", vec![MenuItem::plain(10, "Leaf")]),
            MenuItem::plain(2, "Other"),
        ])
    };
    let mut m = ContextMenu::open(&mut ui, &r, &req).unwrap();
    let items = m.item_elements(0);
    let ev = left_click(&ui, items[0]);
    m.handle_event(&mut ui, &ev);
    let menus = m.menus();
    assert_eq!(menus.len(), 2);
    // A press inside the submenu makes it the active panel.
    ui.activate(menus[1]);
    for ev in ui.take_events() {
        assert_ne!(
            m.handle_event(&mut ui, &ev),
            MenuReply::Event(MenuEvent::Dismissed)
        );
    }
    // Resting on the plain root item closes the submenu.
    ui.set_time(10.0);
    let ev = hover(&ui, items[1], true);
    m.handle_event(&mut ui, &ev);
    ui.set_time(11.0);
    m.update(&mut ui);
    assert_eq!(m.menus().len(), 1);
    assert_eq!(ui.active_panel(), Some(menus[0]), "the root menu is active");
    for ev in ui.take_events() {
        assert_ne!(
            m.handle_event(&mut ui, &ev),
            MenuReply::Event(MenuEvent::Dismissed)
        );
    }
    let mut got = None;
    for ev in real_click(&mut ui, panel) {
        if let MenuReply::Event(e) = m.handle_event(&mut ui, &ev) {
            got = Some(e);
        }
    }
    assert_eq!(got, Some(MenuEvent::Dismissed));
    assert!(!m.is_open());
}

/// A shaper whose fonts arrive late: `None` until `ready` is set.
struct LateFonts {
    inner: TestEnv,
    ready: Arc<std::sync::atomic::AtomicBool>,
}

impl tessera_ui::UiEnv for LateFonts {
    fn layout(&mut self, did: Did) -> Option<Arc<tessera_formats::ui_layout::UiLayout>> {
        self.inner.layout(did)
    }
    fn image(&mut self, image: &ImageRef) -> tessera_ui::ImageState {
        self.inner.image(image)
    }
    fn shape_text(
        &mut self,
        r: &tessera_ui::TextRequest<'_>,
    ) -> Option<Arc<tessera_ui::PreparedText>> {
        if self.ready.load(std::sync::atomic::Ordering::Relaxed) {
            self.inner.shape_text(r)
        } else {
            None
        }
    }
}

/// A menu opened before its font is loaded is sized from the templates, then sized again
/// at the same place once the text can be measured.
#[test]
fn a_menu_opened_before_its_font_is_refitted() {
    let ready = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let env = LateFonts {
        inner: env(),
        ready: Arc::clone(&ready),
    };
    let mut ui = Ui::new(Box::new(env), Vocab::default(), (1024, 768));
    let r = menu_refs();
    let req = MenuRequest {
        position: Some(MenuPosition::TopLeft(100, 120)),
        ..MenuRequest::new(sample_items())
    };
    let mut m = ContextMenu::open(&mut ui, &r, &req).unwrap();
    let narrow = ui.layout_box(m.item_elements(0)[0]).unwrap().w();
    assert_eq!(narrow, 6 + TEXT_PAD + (28 - 6), "the template's text box");
    m.update(&mut ui);
    assert_eq!(ui.layout_box(m.item_elements(0)[0]).unwrap().w(), narrow);
    ready.store(true, std::sync::atomic::Ordering::Relaxed);
    m.update(&mut ui);
    let root = m.root().unwrap();
    let items = m.item_elements(0);
    let wide = ui.layout_box(items[0]).unwrap().w();
    assert!(wide > narrow, "{wide} > {narrow}");
    for k in &items {
        assert_eq!(ui.layout_box(*k).unwrap().w(), wide, "common width");
    }
    let d = ui.displayed_box(root);
    assert_eq!((d.l, d.t), (100, 120), "same place");
    assert_eq!(ui.active_panel(), Some(root));
    // Selection still works on the rebuilt menu.
    let ev = left_click(&ui, items[0]);
    assert!(matches!(
        m.handle_event(&mut ui, &ev),
        MenuReply::Event(MenuEvent::Selected { id: 1, .. })
    ));
}

// ----- engine dialogs -------------------------------------------------------------------

fn key(ui: &mut Ui, name: &str) -> bool {
    ui.key_command(&Command::named(name), true, 1.0)
}

/// Feeds the UI's events to `d` and returns the finished descriptions.
fn pump(ui: &mut Ui, d: &mut DialogBoxes) -> Vec<DialogRequest> {
    for ev in ui.take_events() {
        d.handle_event(ui, &ev, &mut NoTables);
    }
    d.update(ui, &mut NoTables);
    d.take_results()
}

#[test]
fn a_confirmation_fills_its_parts_and_answers_yes_or_no() {
    let mut ui = ui(1024, 768);
    let r = dialog_refs();
    let mut d = DialogBoxes::new(r.clone());
    let mut req = DialogRequest::confirmation(7, DialogText::plain("Abandon this quest?"));
    req.yes_text = Some(DialogText::plain("Abandon"));
    d.request(&mut ui, req.clone(), &mut NoTables);
    let (root, _) = d.shown().expect("shown");
    assert_eq!(ui.element_id(root), Some(r.ids.confirmation_field));
    let q = ui.find_descendant(root, r.ids.question).unwrap();
    assert_eq!(ui.text_of(q), "Abandon this quest?");
    let yes = d.button(DialogOutcome::Yes).unwrap();
    assert_eq!(ui.text_of(yes), "Abandon");
    assert!(engine_dialog_shown(&ui));
    assert_eq!(ui.active_panel(), Some(root));
    assert!(
        ui.band().iter().any(|e| e.map == "DialogBoxes"),
        "DialogBoxes pushed while active"
    );
    // A click on the yes button.
    let ev = button(&ui, yes);
    assert!(d.handle_event(&mut ui, &ev, &mut NoTables));
    let out = d.take_results();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].tag, 7);
    assert_eq!(out[0].confirmation_response, Some(true));
    assert_eq!(out[0].outcome, Some(DialogOutcome::Yes));
    assert!(!ui.exists(root) && !engine_dialog_shown(&ui));
    // EscapeKey: no; its release is consumed.
    d.request(&mut ui, req, &mut NoTables);
    assert!(key(&mut ui, "EscapeKey"));
    let out = pump(&mut ui, &mut d);
    assert_eq!(out[0].confirmation_response, Some(false));
    assert!(!out[0].confirmation_timed_out);
}

#[test]
fn a_message_closes_with_ok_on_escape_and_accept() {
    let mut ui = ui(1024, 768);
    let mut d = DialogBoxes::new(dialog_refs());
    d.request(
        &mut ui,
        DialogRequest::message(1, DialogText::plain("Done.")),
        &mut NoTables,
    );
    assert!(key(&mut ui, "EscapeKey"));
    let out = pump(&mut ui, &mut d);
    assert_eq!(out[0].message_response, Some(true));
    d.request(
        &mut ui,
        DialogRequest::message(2, DialogText::plain("Again.")),
        &mut NoTables,
    );
    assert!(key(&mut ui, "AcceptKey"));
    let out = pump(&mut ui, &mut d);
    assert_eq!((out[0].tag, out[0].outcome), (2, Some(DialogOutcome::Ok)));
}

#[test]
fn a_text_input_takes_text_and_answers_done_or_quit() {
    let mut ui = ui(1024, 768);
    let r = dialog_refs();
    let mut d = DialogBoxes::new(r.clone());
    let mut req = DialogRequest::text_input(3, DialogText::plain("Name?"));
    req.activate_and_take_focus = true;
    req.response = "Ab".into();
    req.max_chars = 4;
    d.request(&mut ui, req.clone(), &mut NoTables);
    let entry = d.entry().expect("entry");
    assert_eq!(ui.focus_element(), Some(entry));
    assert_eq!(ui.text_of(entry), "Ab");
    for c in "cdef".chars() {
        ui.text_input(c);
    }
    assert_eq!(ui.text_of(entry), "Abcd", "max chars 4");
    assert!(key(&mut ui, "AcceptInput"));
    let out = pump(&mut ui, &mut d);
    assert_eq!(out[0].response, "Abcd");
    assert_eq!(out[0].outcome, Some(DialogOutcome::Done));
    // Escape: quit with an empty text.
    d.request(&mut ui, req.clone(), &mut NoTables);
    assert!(key(&mut ui, "EscapeKey"));
    let out = pump(&mut ui, &mut d);
    assert_eq!(
        (out[0].response.as_str(), out[0].outcome),
        ("", Some(DialogOutcome::Quit))
    );
    // Without the quit button, Escape gives no result.
    req.display_cancel = false;
    d.request(&mut ui, req, &mut NoTables);
    let quit = d.button(DialogOutcome::Quit).unwrap();
    assert!(!ui.is_visible(quit));
    key(&mut ui, "EscapeKey");
    assert!(pump(&mut ui, &mut d).is_empty());
    let (root, _) = d.shown().expect("still shown");
    assert!(ui.is_visible(root));
    let done = d.button(DialogOutcome::Done).unwrap();
    let ev = button(&ui, done);
    d.handle_event(&mut ui, &ev, &mut NoTables);
    assert_eq!(d.take_results()[0].outcome, Some(DialogOutcome::Done));
}

#[test]
fn one_dialog_at_a_time_with_topmost_and_cannot_be_overridden() {
    let mut ui = ui(1024, 768);
    let mut a = DialogBoxes::new(dialog_refs());
    let mut b = DialogBoxes::new(dialog_refs());
    a.request(
        &mut ui,
        DialogRequest::message(1, DialogText::plain("first")),
        &mut NoTables,
    );
    a.request(
        &mut ui,
        DialogRequest::message(2, DialogText::plain("second")),
        &mut NoTables,
    );
    b.request(
        &mut ui,
        DialogRequest::message(3, DialogText::plain("other owner")),
        &mut NoTables,
    );
    assert_eq!(a.shown().map(|s| s.1.tag), Some(1));
    assert_eq!(a.queued().len(), 1);
    assert!(b.shown().is_none() && b.queued().len() == 1);
    // A topmost request takes the shown dialog down; it goes back to the front.
    let mut top = DialogRequest::message(4, DialogText::plain("urgent"));
    top.topmost = true;
    a.request(&mut ui, top, &mut NoTables);
    assert_eq!(a.shown().map(|s| s.1.tag), Some(4));
    assert_eq!(
        a.queued().iter().map(|r| r.tag).collect::<Vec<_>>(),
        vec![1, 2]
    );
    // Another owner's topmost request takes it down too; `a` requeues it at its update.
    let mut top_b = DialogRequest::message(5, DialogText::plain("urgent b"));
    top_b.topmost = true;
    b.request(&mut ui, top_b, &mut NoTables);
    assert_eq!(b.shown().map(|s| s.1.tag), Some(5));
    a.update(&mut ui, &mut NoTables);
    assert!(a.shown().is_none());
    assert_eq!(a.queued()[0].tag, 4);
    // A shown dialog that cannot be overridden: a topmost request waits at the front.
    let ok = b.button(DialogOutcome::Ok).unwrap();
    let ev = button(&ui, ok);
    b.handle_event(&mut ui, &ev, &mut NoTables);
    b.take_results();
    // `b` now shows its own next request (3); finish it.
    if b.shown().is_some() {
        let ok = b.button(DialogOutcome::Ok).unwrap();
        let ev = button(&ui, ok);
        b.handle_event(&mut ui, &ev, &mut NoTables);
    }
    a.update(&mut ui, &mut NoTables);
    assert_eq!(a.shown().map(|s| s.1.tag), Some(4));
    let mut locked = DialogRequest::message(6, DialogText::plain("locked"));
    locked.cannot_be_overridden = true;
    let mut c = DialogBoxes::new(dialog_refs());
    // Clear `a` first so that `c` shows its locked dialog.
    let ok = a.button(DialogOutcome::Ok).unwrap();
    let ev = button(&ui, ok);
    a.handle_event(&mut ui, &ev, &mut NoTables);
    a.close_all(&mut ui);
    c.request(&mut ui, locked, &mut NoTables);
    assert_eq!(c.shown().map(|s| s.1.tag), Some(6));
    let mut top = DialogRequest::message(7, DialogText::plain("top"));
    top.topmost = true;
    c.request(
        &mut ui,
        DialogRequest::message(8, DialogText::plain("plain")),
        &mut NoTables,
    );
    c.request(&mut ui, top, &mut NoTables);
    assert_eq!(c.shown().map(|s| s.1.tag), Some(6));
    assert_eq!(
        c.queued().iter().map(|r| r.tag).collect::<Vec<_>>(),
        vec![7, 8]
    );
}

/// Renders the countdown variable into the text.
struct Countdown;

impl TextSource for Countdown {
    fn render(&mut self, _: u32, _: u32, args: &[(u32, String)]) -> Option<String> {
        let n = args
            .iter()
            .find(|(id, _)| *id == COUNTDOWN_VARIABLE)
            .map_or("?", |(_, v)| v.as_str());
        Some(format!("Closing in {n}"))
    }
}

#[test]
fn a_countdown_renders_the_remaining_seconds_and_times_out() {
    let mut ui = ui(1024, 768);
    let r = dialog_refs();
    let mut d = DialogBoxes::new(r.clone());
    ui.set_time(100.0);
    let mut req = DialogRequest::confirmation(9, DialogText::Ref(TextRef::new(1, 2)));
    req.duration = Some(3.0);
    d.request(&mut ui, req.clone(), &mut Countdown);
    let (root, _) = d.shown().unwrap();
    let q = ui.find_descendant(root, r.ids.question).unwrap();
    assert_eq!(ui.text_of(q), "Closing in 3");
    ui.set_time(100.5);
    d.update(&mut ui, &mut Countdown);
    assert_eq!(ui.text_of(q), "Closing in 2");
    // A stall of 5 s moves the end later by the gap.
    ui.set_time(105.5);
    d.update(&mut ui, &mut Countdown);
    assert!(
        d.shown().is_some(),
        "the countdown did not run during the stall"
    );
    ui.set_time(106.1);
    d.update(&mut ui, &mut Countdown);
    assert_eq!(ui.text_of(q), "Closing in 1");
    for t in [107.0, 107.9, 108.05] {
        ui.set_time(t);
        d.update(&mut ui, &mut Countdown);
    }
    let out = d.take_results();
    assert_eq!(out[0].confirmation_response, Some(false));
    assert!(out[0].confirmation_timed_out);
    assert_eq!(out[0].outcome, Some(DialogOutcome::TimedOut));
    // A duration of at most 0.0002 s: no element, the timeout at once.
    req.duration = Some(-0.0001);
    d.request(&mut ui, req, &mut Countdown);
    assert!(d.shown().is_none());
    assert!(d.take_results()[0].confirmation_timed_out);
}

#[test]
fn start_inactive_keeps_the_active_panel_and_modal_blocks_clicks_behind() {
    let mut ui = ui(1024, 768);
    let panel = ui.open_panel(Did(PANEL_LAYOUT), PANEL).unwrap();
    ui.activate(panel);
    let mut d = DialogBoxes::new(dialog_refs());
    let mut req = DialogRequest::message(1, DialogText::plain("quiet"));
    req.start_inactive = true;
    d.request(&mut ui, req, &mut NoTables);
    assert_eq!(ui.active_panel(), Some(panel));
    d.close_all(&mut ui);
    let mut req = DialogRequest::message(2, DialogText::plain("modal"));
    req.modal = true;
    d = DialogBoxes::new(dialog_refs());
    d.request(&mut ui, req, &mut NoTables);
    settle(&mut ui);
    let hit = ui.hit_test(700, 550);
    assert!(
        hit.is_some_and(|h| !ui.is_ancestor_or_self(panel, h)),
        "the panel behind is covered: {hit:?}"
    );
    assert!(tessera_ui::game::dialog::modal_dialog_shown(&ui));
}

#[test]
fn the_fallback_dialog_works_without_a_layout() {
    let mut ui = ui(1024, 768);
    let mut d = DialogBoxes::new(DialogRefs::default());
    let mut req = DialogRequest::text_input(1, DialogText::plain("Who?"));
    req.activate_and_take_focus = true;
    d.request(&mut ui, req, &mut NoTables);
    let (root, _) = d.shown().unwrap();
    let b = ui.displayed_box(root);
    assert_eq!((b.l, b.t, b.w(), b.h()), (256, 294, 512, 180), "centred");
    ui.text_input('Z');
    assert!(key(&mut ui, "AcceptInput"));
    let out = pump(&mut ui, &mut d);
    assert_eq!(out[0].response, "Z");
    assert_eq!(out[0].kind, DialogKind::TextInput);
}

/// `set_refs` with other element IDs: the next dialog's buttons answer real clicks (their
/// handlers are registered), not only the keys.
#[test]
fn new_names_register_their_button_handlers() {
    let mut ui = ui(1024, 768);
    let mut d = DialogBoxes::new(DialogRefs::default());
    let mut req = DialogRequest::text_input(1, DialogText::plain("Who?"));
    req.activate_and_take_focus = true;
    d.request(&mut ui, req.clone(), &mut NoTables);
    let quit = d.button(DialogOutcome::Quit).unwrap();
    for ev in real_click(&mut ui, quit) {
        d.handle_event(&mut ui, &ev, &mut NoTables);
    }
    assert_eq!(d.take_results()[0].outcome, Some(DialogOutcome::Quit));
    // The install's names arrive (other IDs).
    let mut refs = DialogRefs::default();
    refs.ids.text_input_field = 0x7D40_0001;
    refs.ids.done = 0x7D40_0002;
    refs.ids.quit = 0x7D40_0003;
    refs.ids.entry = 0x7D40_0004;
    d.set_refs(refs.clone());
    d.update(&mut ui, &mut NoTables);
    d.request(&mut ui, req, &mut NoTables);
    let (root, _) = d.shown().unwrap();
    assert_eq!(ui.element_id(root), Some(refs.ids.text_input_field));
    ui.text_input('Q');
    let done = d.button(DialogOutcome::Done).unwrap();
    assert_eq!(ui.element_id(done), Some(refs.ids.done));
    let mut got = Vec::new();
    for ev in real_click(&mut ui, done) {
        d.handle_event(&mut ui, &ev, &mut NoTables);
        got.extend(d.take_results());
    }
    assert_eq!(
        got.len(),
        1,
        "the click on the new done button reached the dialog"
    );
    assert_eq!(
        (got[0].outcome, got[0].response.as_str()),
        (Some(DialogOutcome::Done), "Q")
    );
}

/// A dialog the core hid on a key whose result event was lost is shown again at the next
/// update, and stays answerable; later requests are not stuck behind it.
#[test]
fn a_dialog_hidden_without_a_result_is_shown_again() {
    let mut ui = ui(1024, 768);
    let mut d = DialogBoxes::new(dialog_refs());
    let mut req = DialogRequest::text_input(1, DialogText::plain("Name?"));
    req.activate_and_take_focus = true;
    d.request(&mut ui, req.clone(), &mut NoTables);
    let (root, _) = d.shown().unwrap();
    assert!(key(&mut ui, "AcceptInput"));
    assert!(!ui.is_visible(root), "the core hid it");
    // Released before the next press (two presses without one are an OS repeat).
    let _ = release(&mut ui, "AcceptInput");
    let _lost = ui.take_events();
    d.update(&mut ui, &mut NoTables);
    assert_eq!(d.shown().map(|s| s.0), Some(root));
    assert!(ui.is_visible(root));
    assert_eq!(ui.active_panel(), Some(root));
    assert_eq!(ui.focus_element(), d.entry());
    // Keys work again once their events arrive.
    assert!(key(&mut ui, "AcceptInput"));
    let out = pump(&mut ui, &mut d);
    assert_eq!(out[0].outcome, Some(DialogOutcome::Done));
    d.request(&mut ui, req, &mut NoTables);
    assert!(d.shown().is_some_and(|(k, _)| ui.is_visible(k)));
}

fn release(ui: &mut Ui, name: &str) -> bool {
    ui.key_command(&Command::named(name), false, 0.0)
}

/// Whether the UI broadcast a key since the last call (it reaches plugins and game code).
fn broadcast(ui: &mut Ui) -> bool {
    ui.take_events()
        .iter()
        .any(|e| matches!(e, UiEvent::KeyBroadcast { .. }))
}

/// Shows `req` and returns the shown dialog's element; the events so far are dropped.
fn show(ui: &mut Ui, d: &mut DialogBoxes, req: DialogRequest) -> ElementKey {
    d.request(ui, req, &mut NoTables);
    let _ = ui.take_events();
    d.shown().expect("shown").0
}

/// §18.4: a confirmation gives yes on `AcceptKey` and `AcceptInput` and no on `EscapeKey`.
/// Revision 5 pairing rule (Tessera rule): the release of a key whose press gave the result
/// is consumed, after the dialog is gone too (`AcceptInput` included), and the release of a
/// key pressed before the confirmation appeared goes past it while it is shown.
#[test]
fn confirmation_keys_and_releases_follow_18_4() {
    let mut ui = ui(1024, 768);
    let mut d = DialogBoxes::new(dialog_refs());
    let req = || DialogRequest::confirmation(1, DialogText::plain("Sure?"));
    for (name, want, release_consumed) in [
        ("AcceptKey", DialogOutcome::Yes, true),
        ("AcceptInput", DialogOutcome::Yes, true),
        ("EscapeKey", DialogOutcome::No, true),
    ] {
        let root = show(&mut ui, &mut d, req());
        assert!(key(&mut ui, name), "{name} taken");
        let out = pump(&mut ui, &mut d);
        assert_eq!(out[0].outcome, Some(want), "{name}");
        assert!(!ui.exists(root), "{name}: closed");
        assert_eq!(release(&mut ui, name), release_consumed, "{name} release");
        assert_eq!(
            broadcast(&mut ui),
            !release_consumed,
            "{name} release broadcast"
        );
    }
    // Pressed before the confirmation appeared, released while it is shown: the releases
    // go past it to their receiver, the key broadcast included (revision 5 item 3).
    for name in ["AcceptKey", "EscapeKey", "AcceptInput"] {
        assert!(!key(&mut ui, name), "{name}: no dialog yet");
        assert!(broadcast(&mut ui));
    }
    let _ = show(&mut ui, &mut d, req());
    for name in ["AcceptKey", "EscapeKey", "AcceptInput"] {
        assert!(!release(&mut ui, name), "{name} release goes past");
        assert!(broadcast(&mut ui), "{name} release broadcast");
    }
    assert!(pump(&mut ui, &mut d).is_empty(), "releases give no result");
    d.close_all(&mut ui);
}

/// §18.4 revision 5 pairing rule ("the pairing covers every key a dialog takes"): a shown
/// confirmation takes `FocusNext` and `FocusPrevious` for its focus cycle, so neither half
/// of such a key reaches the broadcast; one pressed before the confirmation appeared goes
/// past it at its release.
#[test]
fn a_dialogs_focus_cycle_keys_are_paired() {
    let mut ui = ui(1024, 768);
    let mut d = DialogBoxes::new(dialog_refs());
    let req = || DialogRequest::confirmation(1, DialogText::plain("Sure?"));
    let _ = show(&mut ui, &mut d, req());
    for name in ["FocusNext", "FocusPrevious"] {
        assert!(key(&mut ui, name), "{name} taken by the dialog");
        assert!(!broadcast(&mut ui), "{name} press not broadcast");
        assert!(release(&mut ui, name), "{name} release consumed");
        assert!(!broadcast(&mut ui), "{name} release not broadcast");
    }
    d.close_all(&mut ui);
    let _ = ui.take_events();
    assert!(!key(&mut ui, "FocusNext"), "no dialog yet");
    assert!(broadcast(&mut ui));
    let _ = show(&mut ui, &mut d, req());
    assert!(
        !release(&mut ui, "FocusNext"),
        "a release from before goes past"
    );
    assert!(broadcast(&mut ui), "and is broadcast");
    d.close_all(&mut ui);
}

/// §18.4: a message gives OK on `AcceptKey`, `AcceptInput` and `EscapeKey`. Revision 5
/// pairing rule: the release of each of them is consumed when its press gave the result
/// (the `EscapeKey` of a message included), and goes past a shown message when the key went
/// down before it.
#[test]
fn message_keys_and_releases_follow_18_4() {
    let mut ui = ui(1024, 768);
    let mut d = DialogBoxes::new(dialog_refs());
    let req = || DialogRequest::message(1, DialogText::plain("Done."));
    for name in ["AcceptKey", "AcceptInput", "EscapeKey"] {
        let _ = show(&mut ui, &mut d, req());
        assert!(key(&mut ui, name), "{name} taken");
        let out = pump(&mut ui, &mut d);
        assert_eq!(out[0].outcome, Some(DialogOutcome::Ok), "{name}");
        assert!(release(&mut ui, name), "{name} release consumed");
        assert!(
            !broadcast(&mut ui),
            "{name}: neither half reaches game code"
        );
    }
    for name in ["AcceptKey", "EscapeKey"] {
        assert!(!key(&mut ui, name));
    }
    let _ = show(&mut ui, &mut d, req());
    assert!(
        !release(&mut ui, "AcceptKey"),
        "shown: the AcceptKey release of an earlier press goes past"
    );
    assert!(
        !release(&mut ui, "EscapeKey"),
        "shown: the EscapeKey release of an earlier press goes past"
    );
    d.close_all(&mut ui);
}

/// §18.4: a text input gives done only on `AcceptInput` in its entry (not on `AcceptKey`),
/// and quit on `EscapeKey` in its entry only while the quit button is shown; with the quit
/// button hidden `EscapeKey` is the entry's own key and reaches nothing outside the dialog
/// (revision 5, UI core (f)). The release of a key the dialog took is consumed (pairing
/// rule); other releases go on.
#[test]
fn text_input_keys_follow_18_4() {
    let mut ui = ui(1024, 768);
    let mut d = DialogBoxes::new(dialog_refs());
    let mut req = DialogRequest::text_input(1, DialogText::plain("Name?"));
    req.activate_and_take_focus = true;
    let root = show(&mut ui, &mut d, req.clone());
    key(&mut ui, "AcceptKey");
    assert!(pump(&mut ui, &mut d).is_empty(), "AcceptKey gives no done");
    assert!(ui.is_visible(root));
    assert!(!release(&mut ui, "AcceptKey"));
    // Not in the entry: the entry without the focus reports nothing.
    ui.set_focus(None);
    key(&mut ui, "AcceptInput");
    assert!(
        pump(&mut ui, &mut d).is_empty(),
        "not reported by the entry"
    );
    let _ = release(&mut ui, "AcceptInput");
    ui.set_focus(d.entry());
    assert!(key(&mut ui, "AcceptInput"));
    assert_eq!(pump(&mut ui, &mut d)[0].outcome, Some(DialogOutcome::Done));
    assert!(
        release(&mut ui, "AcceptInput"),
        "the release of the done key"
    );
    assert!(!broadcast(&mut ui));
    // The quit button hidden: EscapeKey gives no result and the dialog stays shown.
    req.display_cancel = false;
    let root = show(&mut ui, &mut d, req.clone());
    assert!(key(&mut ui, "EscapeKey"), "consumed by the entry");
    let ev = ui.take_events();
    assert!(
        !ev.iter().any(|e| matches!(e, UiEvent::DialogResult { .. })),
        "no result from the core"
    );
    assert!(
        !ev.iter().any(|e| matches!(e, UiEvent::KeyBroadcast { .. })),
        "nothing outside the dialog"
    );
    assert!(pump(&mut ui, &mut d).is_empty());
    assert!(ui.is_visible(root), "never hidden");
    assert!(release(&mut ui, "EscapeKey"), "its release too");
    assert!(!broadcast(&mut ui));
    d.close_all(&mut ui);
    let _ = d.take_results();
    // Shown: EscapeKey quits.
    req.display_cancel = true;
    let _ = show(&mut ui, &mut d, req);
    assert!(key(&mut ui, "EscapeKey"));
    assert_eq!(pump(&mut ui, &mut d)[0].outcome, Some(DialogOutcome::Quit));
    assert!(release(&mut ui, "EscapeKey"), "the release of the quit key");
}

/// §18.4: a modal dialog consumes every other key press and the release of every key
/// pressed while it was shown (none reaches the broadcast); a key held from before it
/// appeared is released as usual, and after it closes keys go on as before. A press whose
/// target lies outside the modal dialog is not delivered to it (Tessera rule).
#[test]
fn a_modal_dialog_consumes_every_other_key() {
    let mut ui = ui(1024, 768);
    let panel = ui.open_panel(Did(PANEL_LAYOUT), PANEL).unwrap();
    ui.activate(panel);
    let kp = ui.vocab().msgs.key_press;
    ui.register_handler(PANEL, kp);
    // Held from before the dialog.
    assert!(!key(&mut ui, "MoveLeft"));
    assert!(broadcast(&mut ui));
    let mut d = DialogBoxes::new(dialog_refs());
    let mut req = DialogRequest::confirmation(1, DialogText::plain("Sure?"));
    req.modal = true;
    req.start_inactive = true;
    let root = show(&mut ui, &mut d, req);
    assert_eq!(ui.modal_dialog(), Some(root));
    assert_eq!(ui.active_panel(), Some(panel), "started inactive");
    assert!(key(&mut ui, "MoveForward"), "consumed");
    let ev = ui.take_events();
    assert!(
        !ev.iter().any(|e| matches!(e, UiEvent::KeyBroadcast { .. })),
        "no broadcast"
    );
    assert!(
        !ev.iter()
            .any(|e| matches!(e, UiEvent::Game { message } if message.msg == kp)),
        "not delivered to the panel below"
    );
    assert!(release(&mut ui, "MoveForward"), "its release too");
    assert!(!broadcast(&mut ui));
    // Its OS repeats are presses (consumed), but its release stays game code's.
    assert!(key(&mut ui, "MoveLeft"), "a repeat is consumed");
    assert!(!broadcast(&mut ui));
    assert!(
        !release(&mut ui, "MoveLeft"),
        "held from before: released as usual"
    );
    assert!(broadcast(&mut ui));
    // Pressed while shown, released after the dialog closed: still consumed.
    assert!(key(&mut ui, "Jump"));
    ui.activate(root);
    assert!(key(&mut ui, "AcceptKey"));
    assert_eq!(pump(&mut ui, &mut d)[0].outcome, Some(DialogOutcome::Yes));
    assert_eq!(ui.modal_dialog(), None);
    assert!(release(&mut ui, "Jump"));
    assert!(!broadcast(&mut ui));
    // After it: keys go on as before.
    assert!(!key(&mut ui, "Jump"));
    assert!(broadcast(&mut ui));
    assert!(!release(&mut ui, "Jump"));
}

/// Review finding: an OS repeat of `AcceptKey` after its press gave a confirmation's yes is
/// consumed and delivered nowhere (game code never saw the key go down): it neither drops the
/// held release nor answers the next confirmation, and the release is consumed (§18.4).
#[test]
fn a_repeat_after_a_dialog_result_keeps_its_release_consumed() {
    let mut ui = ui(1024, 768);
    let mut d = DialogBoxes::new(dialog_refs());
    let req = || DialogRequest::confirmation(1, DialogText::plain("Sure?"));
    let _ = show(&mut ui, &mut d, req());
    assert!(key(&mut ui, "AcceptKey"));
    assert_eq!(pump(&mut ui, &mut d)[0].outcome, Some(DialogOutcome::Yes));
    assert!(key(&mut ui, "AcceptKey"), "the repeat is consumed");
    assert!(!broadcast(&mut ui), "the repeat is not broadcast");
    // The next confirmation shown while the key is still down is not answered by a repeat.
    let next = show(&mut ui, &mut d, req());
    assert!(key(&mut ui, "AcceptKey"), "a second repeat is consumed");
    assert!(pump(&mut ui, &mut d).is_empty(), "no result from a repeat");
    assert!(ui.is_visible(next));
    assert!(release(&mut ui, "AcceptKey"), "the release is consumed");
    assert!(!broadcast(&mut ui));
    d.close_all(&mut ui);
}

/// Review finding: a key pressed while a modal dialog was shown keeps its release consumed
/// after the dialog closed even when OS repeats arrive in between, and the repeats are
/// consumed too (§18.4).
#[test]
fn a_repeat_after_a_modal_dialog_closed_keeps_its_release_consumed() {
    let mut ui = ui(1024, 768);
    let mut d = DialogBoxes::new(dialog_refs());
    let mut req = DialogRequest::confirmation(1, DialogText::plain("Sure?"));
    req.modal = true;
    let root = show(&mut ui, &mut d, req);
    assert!(key(&mut ui, "Jump"));
    ui.activate(root);
    assert!(key(&mut ui, "AcceptKey"));
    assert_eq!(pump(&mut ui, &mut d)[0].outcome, Some(DialogOutcome::Yes));
    assert_eq!(ui.modal_dialog(), None);
    assert!(key(&mut ui, "Jump"), "the repeat is consumed");
    assert!(!broadcast(&mut ui));
    assert!(release(&mut ui, "Jump"), "the release is consumed");
    assert!(!broadcast(&mut ui));
    assert!(release(&mut ui, "AcceptKey"));
    assert!(!broadcast(&mut ui));
    // After both: keys go on as before.
    assert!(!key(&mut ui, "Jump"));
    assert!(broadcast(&mut ui));
    assert!(!release(&mut ui, "Jump"));
}

/// Review finding: an OS repeat of a key that went down before a dialog was shown gives no
/// result and holds no release: game code saw the press, so it sees the release once the
/// dialog is gone.
#[test]
fn a_repeat_of_a_key_down_before_the_dialog_gives_no_result() {
    let mut ui = ui(1024, 768);
    let mut d = DialogBoxes::new(dialog_refs());
    assert!(!key(&mut ui, "AcceptKey"), "no dialog: not consumed");
    assert!(broadcast(&mut ui));
    let root = show(
        &mut ui,
        &mut d,
        DialogRequest::confirmation(1, DialogText::plain("Sure?")),
    );
    assert_eq!(ui.active_panel(), Some(root));
    assert!(!key(&mut ui, "AcceptKey"), "the repeat is not the dialog's");
    assert!(broadcast(&mut ui));
    assert!(pump(&mut ui, &mut d).is_empty(), "no result from a repeat");
    assert!(ui.is_visible(root));
    d.close_all(&mut ui);
    let _ = d.take_results();
    let _ = ui.take_events();
    assert!(!release(&mut ui, "AcceptKey"), "the release is not held");
    assert!(broadcast(&mut ui));
}

/// Review finding: while a modal dialog is shown, a typed character whose focused entry lies
/// outside it is taken and inserted nowhere (§18.4: no key reaches anything below it).
#[test]
fn a_modal_dialog_takes_characters_typed_outside_it() {
    let mut ui = ui(1024, 768);
    let w = ui.create_control(ControlKind::Window, 1).unwrap();
    let t = ui.create_control(ControlKind::TextBox, 1).unwrap();
    ui.control_set_parent(t, Some(w)).unwrap();
    ui.activate(w);
    ui.control_focus(t);
    assert!(ui.text_input('a'));
    assert_eq!(ui.text_of(t), "a");
    let mut d = DialogBoxes::new(dialog_refs());
    let mut req = DialogRequest::confirmation(1, DialogText::plain("Sure?"));
    req.modal = true;
    req.start_inactive = true;
    let _ = show(&mut ui, &mut d, req);
    assert!(ui.modal_dialog().is_some());
    assert!(ui.text_input('b'), "taken");
    assert_eq!(ui.text_of(t), "a", "not inserted below the modal dialog");
    let ci = ui.vocab().msgs.character_input;
    assert!(
        !ui.take_events()
            .iter()
            .any(|e| matches!(e, UiEvent::Game { message } if message.msg == ci)),
        "no character input posted"
    );
    d.close_all(&mut ui);
    ui.activate(w);
    ui.control_focus(t);
    assert!(ui.text_input('c'));
    assert_eq!(ui.text_of(t), "ac");
}

/// A non-modal dialog consumes only its own keys (§18.4).
#[test]
fn a_dialog_that_is_not_modal_lets_other_keys_through() {
    let mut ui = ui(1024, 768);
    let mut d = DialogBoxes::new(dialog_refs());
    let _ = show(
        &mut ui,
        &mut d,
        DialogRequest::confirmation(1, DialogText::plain("Sure?")),
    );
    assert_eq!(ui.modal_dialog(), None);
    assert!(!key(&mut ui, "MoveForward"));
    assert!(broadcast(&mut ui));
    assert!(!release(&mut ui, "MoveForward"));
    d.close_all(&mut ui);
}

/// §18.4: `UICore_Dialog_ignorekeyboardinput` concerns confirmations and messages; a text
/// input still reacts to its entry's commands.
#[test]
fn ignore_keyboard_applies_to_confirmations_and_messages_only() {
    let mut ui = ui(1024, 768);
    let mut d = DialogBoxes::new(dialog_refs());
    let mut req = DialogRequest::text_input(1, DialogText::plain("Name?"));
    req.activate_and_take_focus = true;
    req.ignore_keyboard = true;
    d.request(&mut ui, req, &mut NoTables);
    ui.text_input('A');
    assert!(key(&mut ui, "AcceptInput"));
    let out = pump(&mut ui, &mut d);
    assert_eq!(
        (out[0].outcome, out[0].response.as_str()),
        (Some(DialogOutcome::Done), "A")
    );
    let mut req = DialogRequest::confirmation(2, DialogText::plain("Sure?"));
    req.ignore_keyboard = true;
    d.request(&mut ui, req, &mut NoTables);
    assert!(!key(&mut ui, "AcceptKey"), "not taken by the dialog");
    assert!(pump(&mut ui, &mut d).is_empty());
    assert!(d.shown().is_some());
}

/// The queue bound holds for `topmost` requests and for a dialog another owner takes down.
#[test]
fn the_queue_bound_holds_for_topmost_requests() {
    let mut ui = ui(1024, 768);
    let mut a = DialogBoxes::new(dialog_refs());
    a.request(
        &mut ui,
        DialogRequest::message(0, DialogText::plain("shown")),
        &mut NoTables,
    );
    for t in 1..=(MAX_QUEUED as u64 + 3) {
        a.request(
            &mut ui,
            DialogRequest::message(t, DialogText::plain("wait")),
            &mut NoTables,
        );
    }
    assert_eq!(a.queued().len(), MAX_QUEUED);
    for t in 0..(2 * MAX_QUEUED as u64) {
        let mut top = DialogRequest::message(1000 + t, DialogText::plain("top"));
        top.topmost = true;
        a.request(&mut ui, top, &mut NoTables);
        assert!(a.queued().len() <= MAX_QUEUED);
    }
    assert_eq!(
        a.shown().map(|s| s.1.tag),
        Some(0),
        "refused, the shown one stays"
    );
    // Another owner's topmost request takes `a`'s dialog down: back at the front of the
    // full queue, the newest waiting request ends dismissed.
    let mut b = DialogBoxes::new(dialog_refs());
    let mut top = DialogRequest::message(2000, DialogText::plain("b"));
    top.topmost = true;
    b.request(&mut ui, top, &mut NoTables);
    a.update(&mut ui, &mut NoTables);
    assert!(a.shown().is_none());
    assert_eq!(a.queued().len(), MAX_QUEUED);
    assert_eq!(a.queued()[0].tag, 0);
    let out = a.take_results();
    assert_eq!(out.len(), 1);
    assert_eq!(
        (out[0].tag, out[0].outcome),
        (MAX_QUEUED as u64, Some(DialogOutcome::Dismissed))
    );
}

/// The code-made confirmation centres its question both ways (§18.1) and its labels.
#[test]
fn the_fallback_confirmation_centres_its_texts() {
    let mut ui = ui(1024, 768);
    let r = DialogRefs::default();
    let mut d = DialogBoxes::new(r.clone());
    d.request(
        &mut ui,
        DialogRequest::confirmation(1, DialogText::plain("Sure?")),
        &mut NoTables,
    );
    let (root, _) = d.shown().unwrap();
    let centre = tessera_ui::text::JUSTIFY_CENTER;
    let q = ui.find_descendant(root, r.ids.question).unwrap();
    let yes = d.button(DialogOutcome::Yes).unwrap();
    for k in [q, yes] {
        assert_eq!(ui.prop(k, r.pids.text_horizontal).key(), centre);
        assert_eq!(ui.prop(k, r.pids.text_vertical).key(), centre);
    }
}

// ----- goldens --------------------------------------------------------------------------

/// Straight-alpha "over" of `top` onto `base` at (`x`, `y`).
fn blit(base: &mut Rgba, top: &Rgba, x: i32, y: i32) {
    for ty in 0..top.height as i32 {
        for tx in 0..top.width as i32 {
            let (bx, by) = (x + tx, y + ty);
            if bx < 0 || by < 0 || bx >= base.width as i32 || by >= base.height as i32 {
                continue;
            }
            let ti = ((ty as u32 * top.width + tx as u32) * 4) as usize;
            let bi = ((by as u32 * base.width + bx as u32) * 4) as usize;
            let a = u32::from(top.data[ti + 3]);
            for c in 0..3 {
                let s = u32::from(top.data[ti + c]);
                let d = u32::from(base.data[bi + c]);
                base.data[bi + c] = ((s * a + d * (255 - a)) / 255) as u8;
            }
            let da = u32::from(base.data[bi + 3]);
            base.data[bi + 3] = (a + da * (255 - a) / 255).min(255) as u8;
        }
    }
}

#[test]
fn golden_confirmation_dialog() {
    let mut ui = ui(1024, 768);
    let mut d = DialogBoxes::new(dialog_refs());
    let mut req = DialogRequest::confirmation(
        1,
        DialogText::plain("Abandon the quest 'A Lantern in the Fen'?"),
    );
    req.yes_text = Some(DialogText::plain("Yes"));
    req.no_text = Some(DialogText::plain("No"));
    d.request(&mut ui, req, &mut NoTables);
    let (root, _) = d.shown().unwrap();
    golden_file("widgets_confirmation", &capture(&mut ui, root));
}

#[test]
fn golden_nested_menu() {
    let mut ui = ui(640, 480);
    let r = menu_refs();
    let req = MenuRequest {
        title: "Brannoc Fenwright".into(),
        position: Some(MenuPosition::TopLeft(20, 20)),
        ..MenuRequest::new(vec![
            MenuItem::plain(1, "Send tell"),
            MenuItem::plain(2, "Invite to fellowship").enabled(false),
            MenuItem::check(3, "Show vendors", true).with_icon(Some(art(32))),
            MenuItem::submenu(
                4,
                "Group",
                vec![
                    MenuItem::plain(40, "Dismiss"),
                    MenuItem::plain(41, "Make leader"),
                ],
            ),
        ])
    };
    let mut m = ContextMenu::open(&mut ui, &r, &req).unwrap();
    let items = m.item_elements(0);
    let ev = left_click(&ui, items[3]);
    m.handle_event(&mut ui, &ev);
    let ev = hover(&ui, items[3], true);
    m.handle_event(&mut ui, &ev);
    let menus = m.menus();
    assert_eq!(menus.len(), 2);
    settle(&mut ui);
    let a = ui.displayed_box(menus[0]);
    let b = ui.displayed_box(menus[1]);
    let (l, t) = (a.l.min(b.l), a.t.min(b.t));
    let (rr, bb) = (a.r.max(b.r), a.b.max(b.b));
    let mut img = Rgba {
        width: (rr - l) as u32,
        height: (bb - t) as u32,
        data: vec![0; ((rr - l) * (bb - t) * 4) as usize],
    };
    let ca = capture(&mut ui, menus[0]);
    blit(&mut img, &ca, a.l - l, a.t - t);
    let cb = capture(&mut ui, menus[1]);
    blit(&mut img, &cb, b.l - l, b.t - t);
    golden_file("widgets_nested_menu", &img);
}
