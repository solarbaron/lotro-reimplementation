// SPDX-License-Identifier: GPL-3.0-or-later
//! Examination tooltips (`docs/specs/item-tooltips.md` §4, §5, §7–§10, §14) on synthetic
//! trees: row assembly, placement, comparison placement, the examination hover, pins and
//! the name row's font fit. All values are invented; types, element IDs, states and
//! properties are the placeholders of `CharVocab::default()`.

mod ui_common;

use std::collections::HashMap;
use std::sync::Arc;

use proptest::prelude::*;
use tessera_formats::did::Did;
use tessera_formats::property::PropertyValue;
use tessera_formats::ui_layout::UiLayout;
use tessera_ui::events::{Message, MsgArg};
use tessera_ui::game::CharVocab;
use tessera_ui::game::examine::{
    AsmRow, ExamCoin, ExamKey, ExamLine, ExamRow, ExamTemplate, ExamTooltip, ExamTooltips,
    MAX_PINNED, PinOutcome, PinRegistry, RowContent, assemble, place, place_group, satisfies_iv6,
};
use tessera_ui::{ElementKey, MemoryEnv, PreparedText, TextRequest, Ui, UiEnv, UiEvent, Vocab};
use ui_common::*;

const EXAM: u32 = 0x22FF_5000;
const FRAME: u32 = 0x1000_0001;
const LIST: u32 = 0x1000_0002;
const NAME: u32 = 0x1000_0010;
const BIND: u32 = 0x1000_0011;
const SEP: u32 = 0x1000_0012;
const ARMOUR: u32 = 0x1000_0013;
const DUR: u32 = 0x1000_0014;
const DESC: u32 = 0x1000_0015;
const WORTH: u32 = 0x1000_0016;
const LINES: u32 = 0x1000_0017;
const CURRENT: u32 = 0x1000_0018;
const BIND_L: u32 = 0x1000_0021;
const BIND_R: u32 = 0x1000_0022;
const CLOSE: u32 = 0x1000_0030;
const PIN_LIST: u32 = 0x1000_0031;
const HOST_LAYOUT: u32 = 0x22FF_5001;
const SLOT: u32 = 0x1000_0040;
const KIN_LAYOUT: u32 = 0x22FF_5002;
const KFRAME: u32 = 0x1000_0050;
const NAMEICON: u32 = 0x1000_0060;
const ICON: u32 = 0x1000_0061;
const LONG_NAME: &str = "A name far too long for one line of the tooltip";

fn cv() -> CharVocab {
    CharVocab::default()
}

fn templates(ids: &[u32]) -> PropertyValue {
    let p = v().pids;
    array(
        p.lb_templates,
        ids.iter()
            .map(|id| {
                strukt(vec![
                    (p.lb_template_layout, did(EXAM)),
                    (p.lb_template_element, en(*id)),
                ])
            })
            .collect(),
    )
}

/// The §14.1 template order.
const ROWS_14_1: [u32; 12] = [
    NAME, BIND, SEP, ARMOUR, SEP, SEP, DUR, SEP, DESC, SEP, WORTH, SEP,
];

fn exam_layout() -> UiLayout {
    let c = cv();
    let vv = v();
    let t = c.types;
    let mut list = typed(LIST, t.exam_list_box, 5, 5, 229, 90);
    list = prop(list, vv.pids.lb_templates, templates(&ROWS_14_1));
    list = prop(list, vv.pids.lb_autofill, b(true));
    list = prop(list, vv.pids.max_width, int(229));
    list = prop(list, vv.pids.min_width, int(20));
    let frame = child(field(FRAME, 0, 0, 239, 100), list);
    let name = typed(NAME, t.exam_name, 0, 0, 180, 48);
    let mut bind = typed(BIND, t.exam_row, 0, 0, 200, 18);
    bind = prop(bind, c.pids.row_left, en(BIND_L));
    bind = prop(bind, c.pids.row_right, en(BIND_R));
    bind = prop(bind, c.pids.stretch_wide, b(true));
    bind = child(bind, typed(BIND_L, t.exam_binding, 0, 0, 146, 18));
    bind = child(bind, typed(BIND_R, t.exam_unique, 0, 0, 220, 18));
    let sep = prop(field(SEP, 0, 0, 170, 10), c.pids.separator, b(true));
    let armour = typed(ARMOUR, t.exam_armour, 0, 0, 220, 18);
    let dur = state(
        typed(DUR, t.exam_durability, 0, 0, 146, 18),
        0xFFFA_1001,
        0,
        vec![],
        vec![],
    );
    let desc = typed(DESC, t.exam_generic, 0, 0, 220, 18);
    let worth = typed(WORTH, t.exam_value, 0, 0, 204, 22);
    let lines = typed(LINES, t.exam_generic, 0, 0, 220, 0);
    let current = prop(
        typed(CURRENT, vv.types.text, 0, 0, 220, 18),
        vv.pids.text_entry,
        PropertyValue::StringInfo(Box::new(tessera_formats::string_info::StringInfo {
            source: tessera_formats::string_info::StringSource::Literal(
                "Currently equipped".into(),
            ),
            args: None,
        })),
    );
    // The pinned window: a ListBox for the tooltip and a close button.
    let mut pinned = field(c.ids.pinned_tooltip, 0, 0, 250, 140);
    pinned = prop(pinned, c.pids.pinned_offset_x, int(18));
    pinned = prop(pinned, c.pids.pinned_offset_y, int(18));
    pinned = child(pinned, typed(PIN_LIST, vv.types.list_box, 5, 25, 229, 100));
    pinned = child(
        pinned,
        z(typed(CLOSE, vv.types.button, 230, 2, 16, 16), 0, 9),
    );
    // The name-and-icon row (§4.3): a horizontal list of the icon field and the name.
    let mut nameicon = typed(NAMEICON, t.auto_resize_list_box, 0, 0, 229, 48);
    nameicon = prop(nameicon, vv.pids.lb_horizontal, b(true));
    nameicon = prop(nameicon, vv.pids.lb_templates, templates(&[ICON, NAME]));
    let icon = typed(ICON, t.exam_icon, 0, 0, 40, 48);
    layout(
        EXAM,
        1920,
        1080,
        vec![
            frame, name, bind, sep, armour, dur, desc, worth, lines, current, pinned, nameicon,
            icon,
        ],
    )
}

/// A layout of its own holding a tooltip element (as `KinshipCharter_ExamineLayout` in
/// 0x220008BE, §4.1) whose rows come from the examination layout; it has no
/// `PinnedTooltip`.
fn kin_layout() -> UiLayout {
    let c = cv();
    let vv = v();
    let mut list = typed(LIST, c.types.exam_list_box, 5, 5, 229, 90);
    list = prop(list, vv.pids.lb_templates, templates(&[NAME]));
    list = prop(list, vv.pids.max_width, int(229));
    let frame = child(field(KFRAME, 0, 0, 239, 100), list);
    layout(KIN_LAYOUT, 1920, 1080, vec![frame])
}

fn text(extent: (i32, i32)) -> Arc<PreparedText> {
    Arc::new(PreparedText {
        extent,
        ..PreparedText::default()
    })
}

fn env() -> MemoryEnv {
    let mut env = env_with(vec![exam_layout(), host_layout(), kin_layout()]);
    for (s, e) in [
        ("Sword", (120, 40)),
        ("Bind on Equip", (100, 18)),
        ("Unique", (40, 18)),
        ("10 / 10", (60, 18)),
        ("Worth", (30, 14)),
        ("2", (8, 14)),
        ("468", (20, 14)),
        ("line one", (90, 16)),
        ("line two", (110, 16)),
        ("Currently equipped", (130, 18)),
        (LONG_NAME, (400, 16)),
    ] {
        env.texts.insert(s.into(), text(e));
    }
    env
}

fn host_layout() -> UiLayout {
    let slot = prop(field(SLOT, 100, 100, 35, 35), v().pids.tooltips_on, b(true));
    layout(
        HOST_LAYOUT,
        1920,
        1080,
        vec![child(field(0x1000_0041, 0, 0, 1920, 1080), slot)],
    )
}

fn ui() -> Ui {
    let mut ui = ui_with(env(), 1920, 1080);
    cv().register_classes(&mut ui);
    ui
}

fn input_14_1() -> ExamTooltip {
    let rows = vec![
        ExamRow::text("Sword").with_colour(0xFF00_FDFF),
        ExamRow {
            content: RowContent::Pair {
                left: Box::new(ExamRow::text("Bind on Equip")),
                right: Box::new(ExamRow::text("Unique")),
            },
            ..ExamRow::default()
        },
        ExamRow::separator(),
        ExamRow::empty(),
        ExamRow::separator(),
        ExamRow::separator(),
        ExamRow::text("10 / 10").with_state(0xFFFA_1001),
        ExamRow::separator(),
        ExamRow::text(""),
        ExamRow::separator(),
        ExamRow {
            content: RowContent::Money {
                label: Some("Worth".into()),
                coins: vec![
                    ExamCoin {
                        text: "2".into(),
                        icon: None,
                    },
                    ExamCoin {
                        text: "468".into(),
                        icon: None,
                    },
                ],
            },
            ..ExamRow::default()
        },
        ExamRow::separator(),
    ];
    ExamTooltip {
        frame: ExamTemplate {
            layout: EXAM,
            element: FRAME,
        },
        rows,
        comparisons: Vec::new(),
        key: ExamKey {
            instance: 0,
            record: 0x7900_0001,
        },
        templates: None,
    }
}

// ----- §5 on measured rows (§14.1, IV6) ---------------------------------------------------

fn rows_14_1() -> Vec<AsmRow> {
    vec![
        AsmRow::sized(180, 48),
        AsmRow::sized(200, 18),
        AsmRow::separator(170, 10),
        AsmRow::sized(220, 0),
        AsmRow::separator(170, 10),
        AsmRow::separator(170, 10),
        AsmRow::sized(146, 18),
        AsmRow::separator(170, 10),
        AsmRow::sized(220, 0),
        AsmRow::separator(170, 10),
        AsmRow::sized(204, 22),
        AsmRow::separator(170, 10),
    ]
}

#[test]
fn example_14_1_assembly() {
    let rows = rows_14_1();
    let a = assemble(&rows, 229, 20, 229);
    // Name, binding, separator, durability, separator, worth.
    assert_eq!(a.kept, vec![0, 1, 2, 6, 7, 10]);
    assert_eq!(a.width, 204);
    assert!(satisfies_iv6(&rows, &a));
}

#[test]
fn assembly_width_stretch_hidden_and_lists() {
    let mut wide = AsmRow::sized(100, 18);
    wide.stretch_wide = true;
    let mut hidden = AsmRow::sized(50, 18);
    hidden.hidden = true;
    hidden.remove_when_hidden = true;
    let mut kept_hidden = AsmRow::sized(60, 18);
    kept_hidden.hidden = true;
    let empty_list = AsmRow {
        width: 80,
        height: 20,
        list: Some(vec![AsmRow::sized(0, 0), AsmRow::separator(10, 8)]),
        ..AsmRow::default()
    };
    let rows = vec![
        AsmRow::separator(170, 10),
        wide,
        hidden,
        kept_hidden,
        empty_list,
    ];
    let a = assemble(&rows, 229, 20, 229);
    // A separator at the top stays; the hidden row with RemoveWhenHidden goes; a list
    // whose entries are all removed goes (its trailing separator is trimmed).
    assert_eq!(a.kept, vec![0, 1, 3]);
    assert_eq!(a.width, 170);
    assert_eq!(a.stretched, vec![1]);
    // Never wider than max_width, never narrower than min_width.
    assert_eq!(assemble(&[AsmRow::sized(300, 5)], 229, 20, 229).width, 229);
    assert_eq!(assemble(&[AsmRow::sized(5, 5)], 229, 20, 229).width, 20);
    // A list narrower than its widest row is not widened (step 3 only shrinks).
    assert_eq!(assemble(&[AsmRow::sized(200, 5)], 150, 20, 229).width, 150);
    // Nothing left.
    let none = assemble(&[AsmRow::separator(10, 10)], 229, 20, 229);
    assert!(none.kept.is_empty());
}

fn arb_row() -> impl Strategy<Value = AsmRow> {
    (
        0i32..300,
        0i32..60,
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
    )
        .prop_map(|(w, h, sep, hid, rwh)| AsmRow {
            width: w,
            height: h,
            separator: sep,
            hidden: hid,
            remove_when_hidden: rwh,
            stretch_wide: false,
            list: None,
        })
}

proptest! {
    #[test]
    fn iv6_holds_for_any_rows(rows in proptest::collection::vec(arb_row(), 0..60), lw in 0i32..400) {
        let a = assemble(&rows, lw, 20, 229);
        prop_assert!(satisfies_iv6(&rows, &a));
        prop_assert!(a.width <= 229);
        // Kept rows are in order and non-empty.
        prop_assert!(a.kept.windows(2).all(|w| w[0] < w[1]));
        for i in &a.kept {
            prop_assert!(rows[*i].width > 0 && rows[*i].height > 0);
        }
    }
}

// ----- §9 placement (§14.6) and §8.3 comparison placement ----------------------------------

#[test]
fn example_14_6_placement() {
    assert_eq!(
        place((1500, 200), (32, 32), (240, 300), (1920, 1080)),
        (1260, 232)
    );
    assert_eq!(
        place((100, 1000), (32, 32), (240, 300), (1920, 1080)),
        (132, 700)
    );
    // Clamped to the screen; a negative position becomes 0 first.
    assert_eq!(
        place((10, 10), (32, 32), (2000, 50), (1920, 1080)),
        (-80, 42)
    );
    // A screen side that is not positive: the right half, then the clamp.
    assert_eq!(
        place((1910, 1070), (32, 32), (240, 300), (0, 0)),
        (-240, -300)
    );
}

#[test]
fn comparison_group_faces_the_screen_centre() {
    let screen = (1920, 1080);
    // Hover tooltip on the left half: comparisons to its right, tops aligned.
    let g = place_group((100, 200), &[(240, 300), (230, 280), (230, 260)], screen);
    assert_eq!(g, vec![(100, 200), (340, 200), (570, 200)]);
    // On the right half: comparisons to its left, the first next to it.
    let g = place_group((1600, 200), &[(240, 300), (230, 280), (230, 260)], screen);
    assert_eq!(g, vec![(1600, 200), (1370, 200), (1140, 200)]);
    // The group is clamped as one box.
    let g = place_group((1700, 900), &[(240, 300), (230, 400)], screen);
    assert_eq!(g, vec![(1680, 680), (1450, 680)]);
    let g = place_group((10, 10), &[(240, 300), (230, 400)], (300, 1080));
    assert_eq!(g[0].0, -170);
    assert_eq!(place_group((5, 5), &[(10, 10)], screen), vec![(5, 5)]);
    assert!(place_group((5, 5), &[], screen).is_empty());
}

// ----- §10 pin registry ------------------------------------------------------------------

#[test]
fn pin_registry_toggles_and_caps() {
    let mut r: PinRegistry<u32> = PinRegistry::new();
    let k = |i: u32| ExamKey {
        instance: 0,
        record: i,
    };
    for i in 0..MAX_PINNED as u32 {
        assert_eq!(r.decide(k(i)), PinOutcome::Pinned);
        assert!(r.insert(k(i), i));
    }
    assert_eq!(r.decide(k(99)), PinOutcome::Refused);
    assert!(!r.insert(k(99), 99));
    assert_eq!(r.decide(k(3)), PinOutcome::Unpinned);
    assert_eq!(r.remove(k(3)), Some(3));
    assert_eq!(r.decide(k(99)), PinOutcome::Pinned);
    // Instance and record together are the key.
    let inst = ExamKey {
        instance: 7,
        record: 4,
    };
    assert!(!r.contains(inst));
}

// ----- building on the tree --------------------------------------------------------------

fn colour_of(ui: &Ui, k: ElementKey) -> Option<u32> {
    match ui.property(k, cv().pids.text_font_color) {
        Some(PropertyValue::Colour(c)) => Some(u32::from_le_bytes(*c)),
        _ => None,
    }
}

#[test]
fn build_assembles_the_tree_like_example_14_1() {
    let mut ui = ui();
    let tips = ExamTooltips::new(cv());
    let built = tips.build(&mut ui, &input_14_1()).unwrap();
    assert!(built.failures.is_empty(), "{:?}", built.failures);
    let ids: Vec<u32> = built
        .rows
        .iter()
        .map(|k| ui.element_id(*k).unwrap())
        .collect();
    assert_eq!(ids, vec![NAME, BIND, SEP, DUR, SEP, WORTH]);
    // Widest row: the separators (170; the binding pair is 100 + 40 = 140); the list
    // shrinks to it.
    let lb = ui.layout_box(built.list).unwrap();
    assert_eq!((lb.l, lb.t, lb.w()), (5, 5, 170));
    // Heights: 40 + 18 + 10 + 18 + 10 + 16 (worth: coin icons 16 high).
    assert_eq!(lb.h(), 112);
    assert_eq!(built.size, (180, 122));
    assert_eq!(ui.layout_box(built.frame).unwrap().w(), 180);
    // Rows stack from the top.
    let tops: Vec<i32> = built
        .rows
        .iter()
        .map(|k| ui.layout_box(*k).unwrap().t)
        .collect();
    assert_eq!(tops, vec![0, 40, 58, 68, 86, 96]);
    // §4.3: the stretched two-column row puts the right child at the right edge.
    let bind = built.rows[1];
    assert_eq!(ui.layout_box(bind).unwrap().w(), 170);
    let right = ui.find_descendant(bind, BIND_R).unwrap();
    let rb = ui.layout_box(right).unwrap();
    assert_eq!((rb.l, rb.w()), (130, 40));
    // Text, colour and state.
    assert_eq!(ui.text_of(built.rows[0]), "Sword");
    assert_eq!(colour_of(&ui, built.rows[0]), Some(0xFF00_FDFF));
    assert_eq!(ui.state(built.rows[3]), Some(0xFFFA_1001));
    // The worth row: label, count, coin, count, coin.
    assert_eq!(ui.children(built.rows[5]).len(), 5);
    // The frame is a root panel on the screen root; the record is stored on it.
    assert_eq!(ui.parent(built.frame), Some(ui.screen_root()));
    assert_eq!(
        ui.property(built.frame, cv().pids.tooltip_did),
        Some(&PropertyValue::DidRef(Did(0x7900_0001)))
    );
}

#[test]
fn lines_rows_and_literal_tooltip() {
    let mut ui = ui();
    let tips = ExamTooltips::new(cv());
    // A list row of lines, each with its own colour (§6.15 `cantuse` red).
    let mut input = input_14_1();
    input.templates = Some(vec![ExamTemplate {
        layout: EXAM,
        element: LINES,
    }]);
    input.rows = vec![ExamRow {
        content: RowContent::Lines {
            header: None,
            lines: vec![
                ExamLine {
                    text: "line one".into(),
                    state: None,
                    colour: Some(0xFFFF_0000),
                },
                ExamLine {
                    text: "line two".into(),
                    state: None,
                    colour: None,
                },
                ExamLine::default(),
            ],
        },
        colour: Some(0xFFE5_E5E5),
        ..ExamRow::default()
    }];
    let built = tips.build(&mut ui, &input).unwrap();
    assert_eq!(built.rows.len(), 1);
    let lines = ui.children(built.rows[0]);
    assert_eq!(lines.len(), 2, "empty lines are left out");
    assert_eq!(colour_of(&ui, lines[0]), Some(0xFFFF_0000));
    assert_eq!(colour_of(&ui, lines[1]), Some(0xFFE5_E5E5));
    assert_eq!(ui.layout_box(lines[1]).unwrap().t, 16);
    assert_eq!(built.size, (120, 42));
    // §11: the literal tooltip, one name row.
    let lit = ExamTooltip::literal(
        ExamTemplate {
            layout: EXAM,
            element: FRAME,
        },
        ExamTemplate {
            layout: EXAM,
            element: NAME,
        },
        "Sword",
        Some(0xFFFF_FFFF),
    );
    let built = tips.build(&mut ui, &lit).unwrap();
    assert_eq!(built.rows.len(), 1);
    assert_eq!(ui.text_of(built.rows[0]), "Sword");
    assert_eq!(colour_of(&ui, built.rows[0]), Some(0xFFFF_FFFF));
}

#[test]
fn hostile_inputs_do_not_panic() {
    let mut ui = ui();
    let tips = ExamTooltips::new(cv());
    // Missing layout, missing element, a frame without a list box.
    let mut bad = input_14_1();
    bad.frame.layout = 0x22FF_FFFF;
    assert!(tips.build(&mut ui, &bad).is_err());
    bad.frame = ExamTemplate {
        layout: EXAM,
        element: NAME,
    };
    assert!(tips.build(&mut ui, &bad).is_err());
    // Rows longer than the templates, deep nesting, templates naming nothing.
    let mut deep = ExamRow::text("x");
    for _ in 0..64 {
        deep = ExamRow {
            content: RowContent::List(vec![deep]),
            ..ExamRow::default()
        };
    }
    let mut input = input_14_1();
    input.rows = vec![deep; 300];
    input.templates = Some(vec![
        ExamTemplate {
            layout: EXAM,
            element: 0x1234
        };
        3
    ]);
    let built = tips.build(&mut ui, &input).unwrap();
    assert_eq!(built.failures.len(), 3);
    assert!(built.rows.is_empty());
}

// ----- showing, hiding, comparisons (§8, §9) ---------------------------------------------

#[test]
fn show_places_and_registers_with_comparisons() {
    let mut ui = ui();
    let tips = ExamTooltips::new(cv());
    let mut input = input_14_1();
    input.comparisons = vec![input_14_1(), input_14_1()];
    let shown = tips.show(&mut ui, &input, (1500, 200)).unwrap();
    assert_eq!(shown.comparisons.len(), 2);
    // §9: right half → left of the pointer; then the comparisons to its left.
    let (w, h) = shown.hover.size;
    assert_eq!(shown.positions[0], (1500 - w, 232));
    assert_eq!(shown.positions[1], (1500 - w - 180, 232));
    assert_eq!(ui.tooltip_panel(), Some(shown.hover.frame));
    assert_eq!(ui.tooltip_extra_panels().len(), 2);
    let b = ui.layout_box(shown.comparisons[1].frame).unwrap();
    assert_eq!((b.l, b.t, b.w(), b.h()), (1500 - w - 360, 232, 180, h));
    // Hiding (ui-behaviour §6.6) destroys all three.
    ui.hide_tooltip();
    assert!(!ui.exists(shown.hover.frame));
    assert!(shown.comparisons.iter().all(|c| !ui.exists(c.frame)));
    assert!(ui.tooltip_extra_panels().is_empty());
}

#[test]
fn item_slot_hover_requests_the_examination_tooltip() {
    let mut ui = ui();
    let host = ui.open_panel(Did(HOST_LAYOUT), 0x1000_0041).unwrap();
    let slot = ui.find_descendant(host, SLOT).unwrap();
    assert!(!ui.mouse_visible(slot));
    ui.set_examine(slot, true);
    assert!(ui.mouse_visible(slot), "an examining element is hovered");
    ui.pointer_moved(110, 110);
    assert_eq!(ui.hovered(), Some(slot));
    ui.set_time(0.3);
    ui.tick_hover();
    let events = ui.take_events();
    let req = events.iter().find_map(|e| match e {
        UiEvent::ExamineHover { element, pointer } => Some((*element, *pointer)),
        _ => None,
    });
    assert_eq!(req, Some((slot, (110, 110))));
    // The client answers; a pointer move does not hide it, leaving the slot does.
    let tips = ExamTooltips::new(cv());
    let shown = tips.show(&mut ui, &input_14_1(), (110, 110)).unwrap();
    assert_eq!(shown.positions[0], (142, 142));
    ui.pointer_moved(112, 111);
    assert_eq!(ui.tooltip_panel(), Some(shown.hover.frame));
    ui.pointer_moved(5, 5);
    assert!(ui.tooltip_panel().is_none());
    // TooltipEnable false: no request.
    ui.set_tooltip_preferences(false, 0.25);
    ui.pointer_moved(110, 110);
    ui.set_time(5.0);
    ui.tick_hover();
    assert!(
        !ui.take_events()
            .iter()
            .any(|e| matches!(e, UiEvent::ExamineHover { .. }))
    );
}

// ----- §10 pinned tooltips ---------------------------------------------------------------

#[test]
fn pin_toggle_window_offset_limit_and_close() {
    let mut ui = ui();
    let mut tips = ExamTooltips::new(cv());
    let input = input_14_1();
    let shown = tips.show(&mut ui, &input, (100, 100)).unwrap();
    let at = shown.positions[0];
    assert_eq!(
        tips.pin(&mut ui, &input, (100, 100)).unwrap(),
        PinOutcome::Pinned
    );
    let (_, w) = tips.pinned().next().unwrap().clone();
    let wb = ui.layout_box(w.window).unwrap();
    assert_eq!((wb.l, wb.t), (at.0 + 18, at.1 + 18));
    // The window grew by what the tooltip needs beyond its list box (180 × 122 vs 229 × 100).
    assert_eq!((wb.w(), wb.h()), (250, 162));
    assert_eq!(
        ui.list_entries(ui.find_descendant(w.window, PIN_LIST).unwrap()),
        vec![w.tooltip.frame]
    );
    assert_eq!(w.close_buttons.len(), 1);
    // The same item again: the toggle closes it.
    assert_eq!(
        tips.pin(&mut ui, &input, (100, 100)).unwrap(),
        PinOutcome::Unpinned
    );
    assert!(!ui.exists(w.window));
    // 20 at most.
    for i in 0..MAX_PINNED as u64 {
        let mut it = input.clone();
        it.key.instance = i + 1;
        assert_eq!(
            tips.pin(&mut ui, &it, (100, 100)).unwrap(),
            PinOutcome::Pinned
        );
    }
    let mut extra = input.clone();
    extra.key.instance = 999;
    assert_eq!(
        tips.pin(&mut ui, &extra, (100, 100)).unwrap(),
        PinOutcome::Refused
    );
    // The close button's click closes its window.
    let (key, w) = tips.pinned().next().unwrap().clone();
    let click = UiEvent::Game {
        message: Message {
            seq: 0,
            source: w.close_buttons[0],
            source_id: CLOSE,
            msg: ui.vocab().msgs.button_click,
            arg: MsgArg::default(),
        },
    };
    assert!(tips.handle_event(&mut ui, &click));
    assert!(!ui.exists(w.window));
    assert!(!tips.pinned().any(|(k, _)| *k == key));
    assert_eq!(tips.pinned().count(), MAX_PINNED - 1);
}

// ----- §6.2 step 3: the name row's font fit ----------------------------------------------

const F_BIG: u32 = 0x4200_0001;
const F_SMALL: u32 = 0x4200_0002;

/// Text 60 high in every font but `F_SMALL`, where it is 30.
struct FontEnv {
    layouts: HashMap<u32, Arc<UiLayout>>,
}

impl UiEnv for FontEnv {
    fn layout(&mut self, did: Did) -> Option<Arc<UiLayout>> {
        self.layouts.get(&did.0).cloned()
    }

    fn shape_text(&mut self, r: &TextRequest<'_>) -> Option<Arc<PreparedText>> {
        let small = r
            .props
            .iter()
            .any(|(p, v)| *p == cv().pids.text_font && *v == PropertyValue::DidRef(Did(F_SMALL)));
        Some(text((100, if small { 30 } else { 60 })))
    }
}

#[test]
fn name_row_tries_its_fonts_until_one_fits() {
    let vv = v();
    let mut l = exam_layout();
    let fonts = array(
        cv().pids.text_font,
        vec![did(F_BIG), did(F_SMALL), did(F_BIG)],
    );
    for e in &mut l.elements {
        if e.element_id == NAME {
            *e = prop(e.clone(), vv.pids.text_fonts, fonts.clone());
        }
    }
    let env = FontEnv {
        layouts: [(EXAM, Arc::new(l))].into_iter().collect(),
    };
    let mut ui = Ui::new(Box::new(env), Vocab::default(), (1920, 1080));
    cv().register_classes(&mut ui);
    let tips = ExamTooltips::new(cv());
    let mut input = input_14_1();
    input.templates = Some(vec![ExamTemplate {
        layout: EXAM,
        element: NAME,
    }]);
    input.rows = vec![ExamRow::text("A very long name")];
    let built = tips.build(&mut ui, &input).unwrap();
    let name = built.rows[0];
    assert_eq!(
        ui.property(name, cv().pids.text_font),
        Some(&PropertyValue::DidRef(Did(F_SMALL)))
    );
    assert_eq!(ui.layout_box(name).unwrap().h(), 30);
}

#[test]
fn plain_text_rows_keep_their_own_text() {
    // §8.1: every comparison layout starts with `CurrentlyEquipped_Label`, a plain Text
    // with its own entry; an empty input row leaves it as the data gives it.
    let mut ui = ui();
    let tips = ExamTooltips::new(cv());
    let mut input = input_14_1();
    input.templates = Some(
        [CURRENT, NAME, DESC]
            .iter()
            .map(|e| ExamTemplate {
                layout: EXAM,
                element: *e,
            })
            .collect(),
    );
    input.rows = vec![
        ExamRow::empty().with_colour(0xFFFF_7F19),
        ExamRow::text("Sword"),
    ];
    let built = tips.build(&mut ui, &input).unwrap();
    let ids: Vec<u32> = built
        .rows
        .iter()
        .map(|k| ui.element_id(*k).unwrap())
        .collect();
    assert_eq!(ids, vec![CURRENT, NAME]);
    assert_eq!(colour_of(&ui, built.rows[0]), Some(0xFFFF_7F19));
    assert_eq!(built.size, (140, 68));
}

// ----- review regressions ----------------------------------------------------------------

#[test]
fn pinned_list_box_holds_the_whole_tooltip() {
    // The painter clips to the list box: it must be at least the tooltip's size.
    let mut ui = ui();
    let mut tips = ExamTooltips::new(cv());
    let input = input_14_1();
    tips.pin(&mut ui, &input, (100, 100)).unwrap();
    let (_, w) = tips.pinned().next().unwrap().clone();
    let host = ui.find_descendant(w.window, PIN_LIST).unwrap();
    let hb = ui.layout_box(host).unwrap();
    assert_eq!(w.tooltip.size, (180, 122));
    assert!(
        hb.w() >= w.tooltip.size.0 && hb.h() >= w.tooltip.size.1,
        "host {hb:?} smaller than the tooltip {:?}",
        w.tooltip.size
    );
    // The window still grew by the host's growth (229 × 100 → 229 × 122).
    let wb = ui.layout_box(w.window).unwrap();
    assert_eq!((wb.w(), wb.h()), (250, 162));
    // The tooltip is drawn whole: its frame lies inside the host's displayed box.
    let fb = ui.displayed_box(w.tooltip.frame);
    let db = ui.displayed_box(host);
    assert!(fb.l >= db.l && fb.t >= db.t && fb.r <= db.r && fb.b <= db.b);
    assert!(db.w() >= 180 && db.h() >= 122);
}

#[test]
fn pin_opens_the_examination_layouts_window_for_a_tooltip_of_another_layout() {
    let mut ui = ui();
    let mut input = input_14_1();
    input.frame = ExamTemplate {
        layout: KIN_LAYOUT,
        element: KFRAME,
    };
    input.rows = vec![ExamRow::text("Sword")];
    // Without the examination layout the window is looked up in the tooltip's layout,
    // which has none.
    let mut plain = ExamTooltips::new(cv());
    assert!(plain.pin(&mut ui, &input, (100, 100)).is_err());
    let mut tips = ExamTooltips::new(cv()).with_pin_layout(EXAM);
    assert_eq!(
        tips.pin(&mut ui, &input, (100, 100)).unwrap(),
        PinOutcome::Pinned
    );
    let (_, w) = tips.pinned().next().unwrap().clone();
    assert_eq!(ui.element_id(w.window), Some(cv().ids.pinned_tooltip));
    assert_eq!(ui.text_of(w.tooltip.rows[0]), "Sword");
}

fn game_clicks(ui: &mut Ui, button: ElementKey) -> usize {
    ui.take_events();
    let click = ui.vocab().msgs.button_click;
    ui.post(button, click, MsgArg::default());
    ui.take_events()
        .iter()
        .filter(|e| matches!(e, UiEvent::Game { .. }))
        .count()
}

#[test]
fn prune_unregisters_the_close_buttons_of_windows_closed_elsewhere() {
    let mut ui = ui();
    let mut tips = ExamTooltips::new(cv());
    tips.pin(&mut ui, &input_14_1(), (100, 100)).unwrap();
    let (_, w) = tips.pinned().next().unwrap().clone();
    assert_eq!(game_clicks(&mut ui, w.close_buttons[0]), 1);
    // The window is closed by another path (a panel close), then pruned.
    ui.destroy(w.window);
    tips.prune(&mut ui);
    assert_eq!(tips.pinned().count(), 0);
    // A fresh PinnedTooltip window's close button no longer reaches game code.
    let other = ui.open_panel(Did(EXAM), cv().ids.pinned_tooltip).unwrap();
    let close = ui.find_descendant(other, CLOSE).unwrap();
    assert_eq!(game_clicks(&mut ui, close), 0, "leaked registration");
}

#[test]
fn long_name_keeps_the_name_and_icon_row_within_the_list() {
    // §4.3, §5 step 3, §7: the name after the 40-pixel icon wraps within what is left of
    // the list's 229 pixels, so the row never runs past the list box.
    let mut ui = ui();
    let tips = ExamTooltips::new(cv());
    let mut input = input_14_1();
    input.templates = Some(vec![ExamTemplate {
        layout: EXAM,
        element: NAMEICON,
    }]);
    input.rows = vec![ExamRow {
        content: RowContent::List(vec![
            ExamRow {
                content: RowContent::Icon {
                    image: None,
                    ghosted: false,
                },
                ..ExamRow::default()
            },
            ExamRow::text(LONG_NAME),
        ]),
        ..ExamRow::default()
    }];
    let built = tips.build(&mut ui, &input).unwrap();
    assert_eq!(built.rows.len(), 1);
    let row = built.rows[0];
    let rb = ui.layout_box(row).unwrap();
    assert!(rb.w() <= 229, "row {rb:?} wider than the list's max_width");
    let name = ui.find_descendant(row, NAME).unwrap();
    let nb = ui.layout_box(name).unwrap();
    assert_eq!((nb.l, nb.w()), (40, 189));
    assert_eq!(built.size.0, 239);
}

#[test]
fn removed_rows_and_hidden_comparisons_leave_nothing_behind() {
    // The batch destroy of §5 step 1 and of hiding the tooltip panels: everything goes,
    // and the arena is back to what it was.
    let mut ui = ui();
    let tips = ExamTooltips::new(cv());
    let before = ui.element_count();
    let mut input = input_14_1();
    input.templates = Some(
        std::iter::repeat_n(
            ExamTemplate {
                layout: EXAM,
                element: DESC,
            },
            200,
        )
        .chain(std::iter::once(ExamTemplate {
            layout: EXAM,
            element: NAME,
        }))
        .collect(),
    );
    input.rows = (0..200)
        .map(|_| ExamRow::empty())
        .chain(std::iter::once(ExamRow::text("Sword")))
        .collect();
    input.comparisons = vec![input.clone(); 3];
    let shown = tips.show(&mut ui, &input, (100, 100)).unwrap();
    assert_eq!(shown.hover.rows.len(), 1);
    assert_eq!(ui.children(shown.hover.list), shown.hover.rows);
    // Frame, list and the one row per tooltip.
    assert_eq!(ui.element_count(), before + 4 * 3);
    ui.hide_tooltip();
    assert_eq!(ui.element_count(), before);
}
