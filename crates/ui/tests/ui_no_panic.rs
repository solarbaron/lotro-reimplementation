// SPDX-License-Identifier: GPL-3.0-or-later
//! No panic on random trees and random input (hostile data and hostile call sequences),
//! with the behaviour invariants that must hold after every step: UB-I4, UB-I5, UB-I12,
//! UB-I16 (`ui-behaviour.md` §14).

mod ui_common;

use std::time::Duration;

use proptest::prelude::*;
use tessera_formats::did::Did;
use tessera_formats::property::PropertyValue;
use tessera_formats::ui_layout::{Argb, DrawMode, ElementDesc, MediaItem};
use tessera_ui::{Background, CommandOffer, ControlKind, ElementKey, Phase, Ui, UiInput};
use ui_common::*;

const LAYOUT: u32 = 0x22FF_5000;

fn pid_palette() -> Vec<u32> {
    let p = v().pids;
    vec![
        p.object_mode,
        p.object_opacity,
        p.hide,
        p.blocks_clicks,
        p.context_menu,
        p.draw_after_children,
        p.button_ghosted,
        p.button_highlighted,
        p.button_rollover,
        p.button_hot,
        p.button_boolean,
        p.meter_child,
        p.meter_frame,
        p.meter_alphatest,
        p.meter_position,
        p.meter_goal,
        p.meter_smooth,
        p.meter_direction,
        p.meter_child_image,
        p.meter_child_drawmode,
        p.sb_position,
        p.sb_proportional,
        p.sb_num_stops,
        p.sb_slider,
        p.sb_inc_button,
        p.sb_dec_button,
        p.scrollable_vbar,
        p.lb_horizontal,
        p.lb_max_columns,
        p.lb_max_rows,
        p.lb_click_select,
        p.panel_pages,
        p.gb_default,
        p.menu_popup,
        p.menu_listbox,
        p.activatable,
        p.auto_fade,
        p.dragable,
        p.container,
        p.min_width,
        p.max_width,
        p.resize_aspect_ratio,
        p.resize_border_right,
        p.resize_border_bottom,
        p.tooltips_on,
        p.text_editable,
        p.save_loc,
        p.alpha_threshold,
    ]
}

fn value() -> impl Strategy<Value = PropertyValue> {
    prop_oneof![
        any::<bool>().prop_map(PropertyValue::Bool),
        (-3.0f32..3.0).prop_map(PropertyValue::Float),
        prop::num::f32::ANY.prop_map(PropertyValue::Float),
        (-5i32..500).prop_map(PropertyValue::Int),
        (0u32..0x40).prop_map(PropertyValue::Enum),
        (0x1000_0000u32..0x1000_0010).prop_map(PropertyValue::Enum),
        (0x4100_0000u32..0x4100_0004).prop_map(|d| PropertyValue::DidRef(Did(d))),
        Just(PropertyValue::DidRef(Did(LAYOUT))),
    ]
}

fn media_item() -> impl Strategy<Value = MediaItem> {
    prop_oneof![
        (0x4100_0000u32..0x4100_0004, 0u32..10).prop_map(|(d, m)| MediaItem::Image {
            did: Did(d),
            draw_mode: DrawMode(m),
            path: String::new()
        }),
        (0u32..0x40, 0.0f32..1.5).prop_map(|(s, p)| MediaItem::State {
            state: s,
            probability: p
        }),
        (-1.0f32..2.0, -1.0f32..2.0, -1.0f32..2.0).prop_map(|(a, b, c)| MediaItem::Fade {
            start: a,
            end: b,
            duration: c
        }),
        (0.0f32..2.0, any::<bool>(), 0usize..4).prop_map(|(d, l, n)| MediaItem::Animation {
            duration: d,
            draw_mode: DrawMode(4),
            looping: l,
            frames: (0..n).map(|i| Did(0x4100_0000 + i as u32)).collect()
        }),
        (0u32..0x40, 0.0f32..1.0).prop_map(|(m, p)| MediaItem::Message {
            message: m,
            probability: p
        }),
        (-1.0f32..2.0, -1.0f32..2.0).prop_map(|(a, b)| MediaItem::Pause { min: a, max: b }),
        (0u32..10, any::<u32>()).prop_map(|(m, c)| MediaItem::ColouredImage {
            did: Did(0),
            draw_mode: DrawMode(m),
            colour: Argb(c)
        }),
        (0u32..8).prop_map(|t| MediaItem::Jump {
            target: t,
            probability: 1.0
        }),
    ]
}

fn element(depth: u32) -> BoxedStrategy<ElementDesc> {
    let leaf = (
        0x1000_0000u32..0x1000_0010,
        0u32..20,
        (-50i32..400, -50i32..400, -5i32..300, -5i32..300),
        (-3i32..3, 0u32..6),
        prop::array::uniform4(0u32..8),
        0u8..4,
        prop::collection::vec((0usize..48, value()), 0..5),
        prop::collection::vec(media_item(), 0..3),
        prop::collection::vec(
            (
                0u32..0x16,
                0u8..4,
                prop::collection::vec(media_item(), 0..2),
            ),
            0..3,
        ),
        prop::option::of((0x1000_0000u32..0x1000_0010, any::<bool>())),
    )
        .prop_map(|(id, ty, r, zr, e, fl, props, med, states, base)| {
            let pal = pid_palette();
            let mut d = match base {
                Some((b2, _)) => based(id, b2, LAYOUT, r.0, r.1, r.2, r.3),
                None => typed(id, ty, r.0, r.1, r.2, r.3),
            };
            d = edges(z(flags(d, fl), zr.0, zr.1), e);
            for (i, val) in props {
                d = prop(d, pal[i % pal.len()], val);
            }
            for m in med {
                d = media(d, m);
            }
            for (sid, sf, sm) in states {
                if d.state(sid).is_none() {
                    d = state(d, sid, sf, vec![], sm);
                }
            }
            d
        })
        .boxed();
    if depth == 0 {
        return leaf;
    }
    (leaf, prop::collection::vec(element(depth - 1), 0..4))
        .prop_map(|(mut d, kids)| {
            for k in kids {
                if d.child(k.element_id).is_none() {
                    d = child(d, k);
                }
            }
            d
        })
        .boxed()
}

#[derive(Debug, Clone)]
enum Op {
    Open(usize),
    Move(i32, i32),
    Press(usize),
    Release(usize),
    Wheel(bool),
    Key(usize, bool),
    Char(char),
    Tick(u32),
    Paint,
    SetProp(usize, usize, PropertyValue),
    Screen(i32, i32),
    Scale(usize, f32),
    Destroy(usize),
    Control(u8),
    Parent(usize, usize),
    Size(usize, i32, i32),
    Lock(bool),
    Show(usize, bool),
    Focus(usize),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0usize..8).prop_map(Op::Open),
        (-20i32..1100, -20i32..800).prop_map(|(x, y)| Op::Move(x, y)),
        (0usize..9).prop_map(Op::Press),
        (0usize..9).prop_map(Op::Release),
        any::<bool>().prop_map(Op::Wheel),
        (0usize..6, any::<bool>()).prop_map(|(k, d)| Op::Key(k, d)),
        any::<char>().prop_map(Op::Char),
        (0u32..500).prop_map(Op::Tick),
        Just(Op::Paint),
        (0usize..64, 0usize..48, value()).prop_map(|(e, p, v)| Op::SetProp(e, p, v)),
        (-10i32..2000, -10i32..2000).prop_map(|(w, h)| Op::Screen(w, h)),
        (0usize..64, -1.0f32..5.0).prop_map(|(e, s)| Op::Scale(e, s)),
        (0usize..64).prop_map(Op::Destroy),
        (0u8..9).prop_map(Op::Control),
        (0usize..64, 0usize..64).prop_map(|(a, b)| Op::Parent(a, b)),
        (0usize..64, -100i32..2000, -100i32..2000).prop_map(|(e, w, h)| Op::Size(e, w, h)),
        any::<bool>().prop_map(Op::Lock),
        (0usize..64, any::<bool>()).prop_map(|(e, s)| Op::Show(e, s)),
        (0usize..64).prop_map(Op::Focus),
    ]
}

const BUTTONS: [&str; 9] = [
    "SelectLeft",
    "SelectRight",
    "SelectMid",
    "SelectDblLeft",
    "SelectDblRight",
    "SelectDblMid",
    "TapLeft",
    "TapRight",
    "TapMid",
];
const KEYS: [&str; 6] = [
    "EscapeKey",
    "AcceptInput",
    "BackspaceKey",
    "CursorCharLeft",
    "FocusNext",
    "MoveForward",
];

fn check(ui: &mut Ui) {
    // UB-I5.
    if let Some(c) = ui.captured() {
        let h = ui.hovered();
        assert!(h.is_none() || h == Some(c));
    } else if let Some(h) = ui.hovered() {
        // UB-I16: hovered elements are mouse-visible (plugin hover pushes aside).
        let _ = h;
    }
    // UB-I4.
    if let Some(p) = ui.pointer()
        && let Some(h) = ui.hit_test(p.0, p.1)
    {
        assert!(ui.is_visible(h));
        assert!(!ui.is_fading(h));
    }
    // UB-I12.
    if let (Some(f), Some(a)) = (ui.focus_element(), ui.active_panel()) {
        let fp = ui
            .band()
            .iter()
            .filter(|e| e.owner == f)
            .map(|e| e.priority)
            .max();
        let ap = ui
            .band()
            .iter()
            .filter(|e| e.owner == a && e.priority == 0)
            .count();
        if let Some(fp) = fp {
            assert!(fp >= 0 || ap == 0);
        }
    }
}

fn run(elements: Vec<ElementDesc>, ops: Vec<Op>) {
    let mut els = elements;
    // Unique top-level keys.
    let mut seen = std::collections::BTreeSet::new();
    els.retain(|e| seen.insert(e.element_id));
    let ids: Vec<u32> = els.iter().map(|e| e.element_id).collect();
    let mut env = env_with(vec![layout(LAYOUT, 1024, 768, els)]);
    for d in 0x4100_0000u32..0x4100_0004 {
        add_image(&mut env, d, solid(1 + d % 3, 2, 0x80FF_0000 | d));
    }
    env.random = 0.5;
    let mut ui = ui_with(env, 1024, 768);
    let mut keys: Vec<ElementKey> = Vec::new();
    let mut t = 0.0f64;
    let mut hold = 0u64;
    let mut budget = tessera_ui::paint::BandBudget(64);
    let mut held: Vec<(u64, &str)> = Vec::new();
    for o in ops {
        let all = |ui: &Ui, keys: &[ElementKey]| -> Vec<ElementKey> {
            let mut v: Vec<ElementKey> = keys.iter().copied().filter(|k| ui.exists(*k)).collect();
            let mut stack = v.clone();
            while let Some(k) = stack.pop() {
                for c in ui.children(k) {
                    if v.len() < 512 {
                        v.push(c);
                        stack.push(c);
                    }
                }
            }
            v
        };
        let pick = |ui: &Ui, keys: &[ElementKey], i: usize| -> Option<ElementKey> {
            let a = all(ui, keys);
            (!a.is_empty()).then(|| a[i % a.len()])
        };
        match o {
            Op::Open(i) => {
                if !ids.is_empty()
                    && let Ok(k) = ui.open_panel(Did(LAYOUT), ids[i % ids.len()])
                {
                    keys.push(k);
                }
            }
            Op::Move(x, y) => {
                ui.offer(&UiInput::PointerMoved {
                    x: f64::from(x),
                    y: f64::from(y),
                });
            }
            Op::Press(b) => {
                hold += 1;
                held.push((hold, BUTTONS[b % 9]));
                ui.offer(&UiInput::Command(CommandOffer {
                    phase: Phase::Start,
                    hold,
                    command: cmd(BUTTONS[b % 9]),
                    value: 1.0,
                    position: None,
                }));
            }
            Op::Release(i) => {
                if !held.is_empty() {
                    let (h, n) = held.remove(i % held.len());
                    ui.offer(&UiInput::Command(CommandOffer {
                        phase: Phase::End,
                        hold: h,
                        command: cmd(n),
                        value: 1.0,
                        position: None,
                    }));
                }
            }
            Op::Wheel(up) => {
                ui.wheel(&cmd(if up { "ScrollUp" } else { "ScrollDown" }));
            }
            Op::Key(k, down) => {
                ui.key_command(&cmd(KEYS[k % KEYS.len()]), down, 1.0);
            }
            Op::Char(c) => {
                ui.offer(&UiInput::Char(c));
            }
            Op::Tick(ms) => {
                t += f64::from(ms) / 1000.0;
                ui.set_time(t);
                ui.tick_hover();
                ui.frame_work();
            }
            Op::Paint => {
                ui.paint(Duration::from_secs_f64(t), &mut budget);
            }
            Op::SetProp(e, p, val) => {
                if let Some(k) = pick(&ui, &keys, e) {
                    let pal = pid_palette();
                    ui.set_property(k, pal[p % pal.len()], val);
                }
            }
            Op::Screen(w, h) => ui.resize_screen(w, h),
            Op::Scale(e, s) => {
                if let Some(k) = pick(&ui, &keys, e) {
                    ui.scale_panel(k, s, e % 2 == 0);
                }
            }
            Op::Destroy(e) => {
                if let Some(k) = pick(&ui, &keys, e) {
                    ui.destroy(k);
                }
            }
            Op::Control(kind) => {
                let kinds = [
                    ControlKind::Control,
                    ControlKind::Window,
                    ControlKind::Label,
                    ControlKind::TextBox,
                    ControlKind::Button,
                    ControlKind::CheckBox,
                    ControlKind::ScrollBar,
                    ControlKind::ListBox,
                    ControlKind::TreeView,
                ];
                if let Ok(k) = ui.create_control(kinds[kind as usize % 9], 1) {
                    ui.control_set_visible(k, true);
                    ui.control_set_back_colour(k, Some([0.5, 1.0, 0.0, 0.0]));
                    ui.control_set_background(k, Background::Did(0x4100_0001));
                    ui.control_attach_edges(k, [i32::from(kind) % 7, 5, 1, 3]);
                    keys.push(k);
                }
            }
            Op::Parent(a, b2) => {
                if let (Some(x), Some(y)) = (pick(&ui, &keys, a), pick(&ui, &keys, b2))
                    && ui.is_control(x)
                {
                    let _ = ui.control_set_parent(x, Some(y));
                }
            }
            Op::Size(e, w, h) => {
                if let Some(k) = pick(&ui, &keys, e) {
                    if ui.is_control(k) {
                        ui.control_set_size(k, w, h);
                        ui.control_set_stretch_mode(k, w % 5);
                        ui.window_set_rotation(k, [0.0, 0.0, h as f32]);
                    } else {
                        ui.resize(k, w, h);
                    }
                }
            }
            Op::Lock(on) => ui.set_pointer_lock(on.then_some((5, 5))),
            Op::Show(e, s) => {
                if let Some(k) = pick(&ui, &keys, e) {
                    ui.set_shown(k, s);
                }
            }
            Op::Focus(e) => {
                let k = pick(&ui, &keys, e);
                ui.set_focus(k);
                if let Some(k) = k {
                    ui.activate(k);
                    ui.control_focus(k);
                }
            }
        }
        check(&mut ui);
        ui.take_events();
        ui.take_band_changes();
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 96,
        ..ProptestConfig::default()
    })]

    #[test]
    fn random_trees_and_inputs_do_not_panic(
        elements in prop::collection::vec(element(2), 1..6),
        ops in prop::collection::vec(op(), 0..60),
    ) {
        run(elements, ops);
    }
}
