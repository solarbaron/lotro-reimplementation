// SPDX-License-Identifier: GPL-3.0-or-later
//! Worked examples and invariants of `docs/specs/ui-layout.md` (§7–§9, §12, §14) on
//! synthetic trees. All values are invented.

mod ui_common;

use std::sync::Arc;

use tessera_formats::did::Did;
use tessera_formats::property::PropertyValue;
use tessera_formats::string_info::{StringInfo, StringSource};
use tessera_formats::ui_layout::{Argb, DrawMode, MediaItem};
use tessera_formats::{NoSchema, UiLayout};
use tessera_ui::geom::IBox;
use tessera_ui::layout::{RuntimeInput, design_time, runtime};
use tessera_ui::resolve::resolve;
use tessera_ui::{ImageRef, UiEvent, normalized_to_pixels};
use ui_common::*;

fn hex(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).unwrap())
        .collect()
}

/// §14.1: the 158-byte record.
fn record_14_1() -> Vec<u8> {
    let mut v = hex("01 F0 FF 22 20 03 00 00 58 02 00 00 00 01 34 12 00 10 00 00 00 00 01");
    v.extend(hex(
        "0A 00 00 00 14 00 00 00 64 00 00 00 1E 00 00 00 FB FF FF FF 01 00 00 01",
    ));
    v.extend(hex(
        "04 00 00 00 04 00 00 00 EF CD AB 41 04 00 00 00 00 00 00 00 00",
    ));
    v.extend(hex(
        "34 12 00 10 09 00 00 00 00 00 00 00 00 00 00 00 0B 00 00 00",
    ));
    v.extend(hex(
        "05 00 00 00 05 00 00 00 04 00 00 00 04 00 00 00 01 01 0B 00 00 00 0B 00 00 00",
    ));
    v.push(0);
    v.extend([0u8; 20]);
    v.push(0);
    v.extend(hex(
        "00 00 01 04 00 00 00 04 00 00 00 F0 CD AB 41 04 00 00 00 00 00 00",
    ));
    v
}

#[test]
fn example_14_1_record_panel_and_states() {
    let data = record_14_1();
    assert_eq!(data.len(), 158);
    let l = UiLayout::parse(&data, &NoSchema).unwrap();
    let mut ui = ui_with(env_with(vec![l]), 1600, 1200);
    let k = ui.open_panel(Did(0x22FF_F001), 0x1000_1234).unwrap();
    // Edges (5, 5, 4, 4) keep the design box at 1600 × 1200.
    assert_eq!(ui.layout_box(k), Some(IBox::new(10, 20, 109, 49)));
    assert_eq!(ui.class(k), Some(tessera_ui::WidgetClass::Button));
    // Post-creation shows 0x41ABCDEF, then the switch to 0x0B replaces it.
    assert_eq!(ui.state(k), Some(0x0B));
    assert_eq!(ui.base_image(k), Some((ImageRef::Did(0x41AB_CDF0), 4)));
}

#[test]
fn example_14_2_layout_arithmetic() {
    let o = IBox::new(0, 0, 1023, 767);
    let n = IBox::new(0, 0, 1919, 1079);
    type Case = ((i32, i32, i32, i32), [u32; 4], (i32, i32, i32, i32));
    let cases: [Case; 5] = [
        ((824, 668, 200, 100), [4, 4, 5, 5], (1720, 980, 200, 100)),
        ((0, 0, 1024, 40), [5, 5, 5, 4], (0, 0, 1920, 40)),
        ((362, 284, 300, 200), [1, 1, 1, 1], (810, 440, 300, 200)),
        ((100, 50, 200, 100), [3, 3, 3, 3], (187, 70, 374, 140)),
        ((0, 0, 1024, 768), [6, 5, 6, 5], (239, 0, 1440, 1080)),
    ];
    for (d, e, want) in cases {
        let db = IBox::from_xywh(d.0, d.1, d.2, d.3);
        let b = runtime(&RuntimeInput {
            design: db,
            current: db,
            edges: e,
            old: o,
            new: n,
            finished: false,
            keep_inside: false,
        });
        assert_eq!((b.l, b.t, b.w(), b.h()), want, "edges {e:?}");
    }
    // The same through root panels on a 1920 × 1080 screen.
    let els: Vec<_> = cases
        .iter()
        .enumerate()
        .map(|(i, (d, e, _))| edges(field(0x1000_0100 + i as u32, d.0, d.1, d.2, d.3), *e))
        .collect();
    let mut ui = ui_one(0x22FF_0002, 1024, 768, els, 1920, 1080);
    for (i, (_, _, want)) in cases.iter().enumerate() {
        let k = ui
            .open_panel(Did(0x22FF_0002), 0x1000_0100 + i as u32)
            .unwrap();
        let b = ui.layout_box(k).unwrap();
        assert_eq!((b.l, b.t, b.w(), b.h()), *want);
    }
}

#[test]
fn runtime_rule_keep_inside_and_mode_zero() {
    // Step 5 bounds are the parent's width and height (U25): right may be Nw.
    let d = IBox::new(-5, -5, 200, 200);
    let b = runtime(&RuntimeInput {
        design: d,
        current: d,
        edges: [5, 5, 5, 5],
        old: IBox::new(0, 0, 99, 99),
        new: IBox::new(0, 0, 99, 99),
        finished: true,
        keep_inside: true,
    });
    assert_eq!(b, IBox::new(0, 0, 100, 100));
    // Step 4: mode 0 takes the current box.
    let b = runtime(&RuntimeInput {
        design: IBox::new(0, 0, 9, 9),
        current: IBox::new(30, 40, 49, 59),
        edges: [0, 0, 0, 0],
        old: IBox::new(0, 0, 99, 99),
        new: IBox::new(0, 0, 199, 199),
        finished: true,
        keep_inside: false,
    });
    assert_eq!(b, IBox::new(30, 40, 49, 59));
    // Mode 2 uses the design size for left/top and the current size for right/bottom.
    let b = runtime(&RuntimeInput {
        design: IBox::new(10, 10, 89, 89),
        current: IBox::new(10, 10, 89, 89),
        edges: [5, 5, 2, 2],
        old: IBox::new(0, 0, 99, 99),
        new: IBox::new(0, 0, 199, 199),
        finished: false,
        keep_inside: false,
    });
    assert_eq!(b, IBox::new(10, 10, 189, 189));
}

#[test]
fn runtime_rule_rescues_a_degenerate_box() {
    // The loadscreen meter (199 × 14, edges all SAME) in `NormalBackground` once it shrank
    // from its 1024 × 768 design to 597 × 588: steps 2–5 would give −228 × −166 (the
    // numbers of `ui-layout.md` §8.4 step 6 OPEN). The Tessera fallback keeps its design
    // extent and shifts it inside the parent.
    let d = IBox::from_xywh(412, 700, 199, 14);
    let b = runtime(&RuntimeInput {
        design: d,
        current: d,
        edges: [5, 5, 5, 5],
        old: IBox::new(0, 0, 1023, 767),
        new: IBox::new(0, 0, 596, 587),
        finished: false,
        keep_inside: false,
    });
    assert_eq!((b.w(), b.h()), (199, 14), "design extent kept");
    assert!(
        b.l >= 0 && b.t >= 0 && b.r <= 596 && b.b <= 587,
        "inside parent: {b:?}"
    );
    assert_eq!(b, IBox::new(398, 574, 596, 587));
    // A degenerate element whose design is wider than the shrunken parent takes the
    // parent's whole width.
    let d = IBox::from_xywh(0, 0, 5000, 14);
    let b = runtime(&RuntimeInput {
        design: d,
        current: d,
        edges: [5, 5, 5, 5],
        old: IBox::new(0, 0, 9999, 767),
        new: IBox::new(0, 0, 99, 587),
        finished: false,
        keep_inside: false,
    });
    assert_eq!((b.l, b.w()), (0, 100), "clamped to the parent width");
    // A positive box that overflows its parent (a `chargen_appearance` side-panel child in
    // the OPEN) is left exactly as steps 2–5 give it.
    let d = IBox::from_xywh(10, 10, 600, 600);
    let b = runtime(&RuntimeInput {
        design: d,
        current: d,
        edges: [0, 0, 0, 0],
        old: IBox::new(0, 0, 999, 999),
        new: IBox::new(0, 0, 199, 199),
        finished: true,
        keep_inside: false,
    });
    assert_eq!(b, IBox::from_xywh(10, 10, 600, 600), "unchanged");
}

#[test]
fn design_time_rule_skips_modes_2_and_6() {
    let d = IBox::new(5, 5, 114, 24);
    let b = design_time(d, [2, 6, 2, 6], 120, 30, 200, 30);
    assert_eq!(b, d);
    let b = design_time(d, [5, 5, 5, 5], 120, 30, 200, 30);
    assert_eq!(b, IBox::new(5, 5, 194, 24));
}

fn text_entry(s: &str) -> PropertyValue {
    PropertyValue::StringInfo(Box::new(StringInfo {
        source: StringSource::Literal(s.into()),
        args: None,
    }))
}

#[test]
fn example_14_3_inheritance() {
    let vv = v();
    let p = vv.pids;
    let t = typed(0x1000_0010, vv.types.button, 0, 0, 120, 30);
    let t = media(t, image_item(0x4100_0010, 4));
    let t = prop(t, p.text_entry, text_entry("a"));
    let t = prop(t, p.button_rollover, b(true));
    let l = z(edges(field(0x1000_0011, 5, 5, 110, 20), [5, 5, 5, 5]), 0, 1);
    let t = child(t, l);
    let t = state(t, 0x08, 0, vec![], vec![image_item(0x4100_0011, 4)]);
    let lay = layout(0x22FF_0003, 1024, 768, vec![t]);

    let e = based(0x1000_0020, 0x1000_0010, 0x22FF_0003, 40, 60, 200, 30);
    let e = prop(e, p.text_entry, text_entry("b"));
    let e = child(e, z(field(0x1000_0021, 0, 0, 10, 10), 0, 1));
    let e = state(
        e,
        0x08,
        0,
        vec![],
        vec![MediaItem::Sound {
            did: Did(0x2A00_0005),
        }],
    );
    let e = edges(e, [4, 5, 5, 4]);

    let la = Arc::new(lay);
    let r = resolve(&e, &mut |d| (d == Did(0x22FF_0003)).then(|| la.clone())).unwrap();
    assert_eq!(r.element_type, vv.types.button);
    assert_eq!(r.element_id, 0x1000_0020);
    assert_eq!(
        (
            r.block.rect.x,
            r.block.rect.y,
            r.block.rect.width,
            r.block.rect.height
        ),
        (40, 60, 200, 30)
    );
    assert_eq!(r.edges.to_array().map(|e| e.0), [4, 5, 5, 4]);
    assert_eq!(r.block.media, vec![image_item(0x4100_0010, 4)]);
    assert_eq!(r.block.properties.get(p.text_entry), Some(&text_entry("b")));
    assert_eq!(r.block.properties.get(p.button_rollover), Some(&b(true)));
    let s = r.state(0x08).unwrap();
    assert_eq!(
        s.media,
        vec![
            image_item(0x4100_0011, 4),
            MediaItem::Sound {
                did: Did(0x2A00_0005)
            }
        ]
    );
    let lc = r.child(0x1000_0011).unwrap();
    assert_eq!(
        (
            lc.block.rect.x,
            lc.block.rect.y,
            lc.block.rect.width,
            lc.block.rect.height
        ),
        (5, 5, 190, 20)
    );
    assert_eq!(r.child(0x1000_0021).unwrap().read_order, 2);
}

#[test]
fn example_14_5_overriding_a_based_child() {
    let vv = v();
    let x = media(
        typed(0x1000_0300, vv.types.text, 0, 0, 10, 10),
        image_item(0x4100_0030, 4),
    );
    let y = media(
        typed(0x1000_0301, vv.types.text, 0, 0, 10, 10),
        image_item(0x4100_0031, 4),
    );
    let kchild = z(
        media(
            based(0x1000_0500, 0x1000_0300, 0x22FF_F030, 0, 0, 50, 20),
            image_item(0x4100_0040, 4),
        ),
        0,
        1,
    );
    let p = child(field(0x1000_0400, 0, 0, 100, 40), kchild);
    let echild = z(
        media(
            based(0x1000_0500, 0x1000_0301, 0x22FF_F030, 5, 5, 60, 20),
            MediaItem::Sound {
                did: Did(0x2A00_0007),
            },
        ),
        0,
        1,
    );
    let e = child(
        based(0x1000_0401, 0x1000_0400, 0x22FF_F030, 0, 0, 100, 40),
        echild,
    );
    let lay = layout(0x22FF_F030, 1024, 768, vec![x, y, p, e]);
    let la = Arc::new(lay.clone());
    let r = resolve(&lay.elements[3], &mut |d| {
        (d == Did(0x22FF_F030)).then(|| la.clone())
    })
    .unwrap();
    let c = r.child(0x1000_0500).unwrap();
    assert_eq!(
        (c.base_element, c.base_layout),
        (0x1000_0300, Did(0x22FF_F030))
    );
    assert_eq!(
        (
            c.block.rect.x,
            c.block.rect.y,
            c.block.rect.width,
            c.block.rect.height
        ),
        (5, 5, 60, 20)
    );
    assert_eq!(
        c.block.media,
        vec![
            image_item(0x4100_0040, 4),
            MediaItem::Sound {
                did: Did(0x2A00_0007)
            }
        ]
    );
    // Creating E resolves the merged child: type Text, base X's media first.
    let mut ui = ui_with(env_with(vec![lay]), 1024, 768);
    let ek = ui
        .create_element(None, Did(0x22FF_F030), 0x1000_0401)
        .unwrap();
    let ck = ui.find_child(ek, 0x1000_0500).unwrap();
    assert_eq!(ui.class(ck), Some(tessera_ui::WidgetClass::Text));
    assert_eq!(ui.layout_box(ck), Some(IBox::from_xywh(5, 5, 60, 20)));
    assert_eq!(ui.base_image(ck), Some((ImageRef::Did(0x4100_0040), 4)));
    assert!(
        ui.take_events()
            .contains(&UiEvent::Sound { did: 0x2A00_0007 })
    );
}

#[test]
fn example_14_6_drawing_order_and_hit_test() {
    let vv = v();
    let bt = vv.types.button;
    let a = z(typed(0xA, bt, 0, 0, 50, 50), 10, 1);
    let bb = z(typed(0xB, bt, 0, 0, 50, 50), 0, 2);
    let c = z(typed(0xC, bt, 0, 0, 50, 50), 0, 1);
    let e = z(typed(0xE, bt, 0, 0, 50, 50), -5, 3);
    let p = child(
        child(child(child(field(0x1000_0001, 0, 0, 100, 100), a), bb), c),
        e,
    );
    let mut ui = ui_one(0x22FF_0006, 1024, 768, vec![p], 1024, 768);
    let pk = ui.open_panel(Did(0x22FF_0006), 0x1000_0001).unwrap();
    let ids: Vec<u32> = ui
        .children(pk)
        .iter()
        .map(|k| ui.element_id(*k).unwrap())
        .collect();
    assert_eq!(ids, vec![0xA, 0xC, 0xB, 0xE]);
    let hit = ui.hit_test(10, 10).unwrap();
    assert_eq!(ui.element_id(hit), Some(0xE));
    // With E hidden, B (equal z, higher read order than C) is next.
    let ek = ui.find_child(pk, 0xE).unwrap();
    ui.set_shown(ek, false);
    let h = ui.hit_test(10, 10).unwrap();
    assert_eq!(ui.element_id(h), Some(0xB));
}

#[test]
fn draw_after_children_puts_own_content_on_top() {
    for dac in [false, true] {
        let vv = v();
        let p = media(field(0x1000_0001, 0, 0, 20, 20), image_item(0x4100_0001, 2));
        let p = prop(p, vv.pids.draw_after_children, b(dac));
        let c = media(field(0x1000_0002, 0, 0, 10, 10), image_item(0x4100_0002, 2));
        let p = child(p, c);
        let mut env = env_with(vec![layout(0x22FF_0007, 100, 100, vec![p])]);
        add_image(&mut env, 0x4100_0001, solid(20, 20, 0xFFFF_0000));
        add_image(&mut env, 0x4100_0002, solid(10, 10, 0xFF00_FF00));
        let mut ui = ui_with(env, 100, 100);
        let pk = ui.open_panel(Did(0x22FF_0007), 0x1000_0001).unwrap();
        let mut budget = tessera_ui::paint::Unlimited;
        for t in 0..3 {
            ui.paint(std::time::Duration::from_millis(t * 16), &mut budget);
        }
        let px = ui.surface_pixel(pk, 5, 5).unwrap();
        assert_eq!(px, if dac { 0xFFFF_0000 } else { 0xFF00_FF00 }, "dac {dac}");
        assert_eq!(ui.surface_pixel(pk, 15, 15), Some(0xFFFF_0000));
    }
}

#[test]
fn example_14_8_saved_location() {
    assert_eq!(normalized_to_pixels(-0.5, 0.5, 1920, 1080), (480, 269));
    assert_eq!(normalized_to_pixels(0.0, -0.25, 1920, 1080), (960, 674));
    let vv = v();
    let p = vv.pids;
    let e = prop(
        prop(
            prop(field(0x1000_0001, 0, 0, 10, 10), p.save_loc, b(true)),
            p.save_size,
            b(true),
        ),
        p.save_visibility,
        b(true),
    );
    let mut ui = ui_one(0x22FF_0008, 1920, 1080, vec![e], 1920, 1080);
    let k = ui.open_panel(Did(0x22FF_0008), 0x1000_0001).unwrap();
    let mut store = tessera_ui::SavedStore::default();
    store.entries.insert(
        (0x22FF_0008, 0x1000_0001),
        tessera_ui::SavedEntry {
            x0: -0.5,
            y0: 0.5,
            x1: 0.0,
            y1: -0.25,
            visible: 1,
        },
    );
    ui.set_saved_store(store);
    assert!(ui.apply_saved(k));
    let bx = ui.layout_box(k).unwrap();
    assert_eq!((bx.l, bx.t, bx.w(), bx.h()), (480, 269, 481, 406));
    let xml = ui.saved_store().to_xml();
    assert!(
        xml.contains("X=\"0.250000\" Y=\"0.250000\" Width=\"0.250000\" Height=\"0.375000\""),
        "{xml}"
    );
}

#[test]
fn invariant_ul20_corner_conversion() {
    for (w, h) in [
        (1, 1),
        (640, 480),
        (1024, 768),
        (1920, 1080),
        (3840, 2160),
        (7, 13),
    ] {
        assert_eq!(normalized_to_pixels(-1.0, 1.0, w, h), (0, 0), "{w}x{h}");
    }
}

#[test]
fn example_14_4_button_states() {
    let vv = v();
    let st = vv.states;
    let mut btn = typed(0x1000_0001, vv.types.button, 10, 10, 50, 20);
    btn = prop(btn, vv.pids.button_rollover, b(true));
    for s in [st.normal, st.normal_rollover, st.normal_pressed] {
        btn = state(btn, s, 0, vec![], vec![]);
    }
    let mut ui = ui_one(0x22FF_0009, 200, 200, vec![btn], 200, 200);
    let k = ui.open_panel(Did(0x22FF_0009), 0x1000_0001).unwrap();
    assert_eq!(ui.state(k), Some(st.normal));
    ui.pointer_moved(20, 15);
    assert_eq!(ui.state(k), Some(st.normal_rollover));
    ui.mouse_press(&cmd("SelectLeft"));
    assert_eq!(ui.state(k), Some(st.normal_pressed));
    ui.pointer_moved(150, 150);
    assert_eq!(ui.state(k), Some(st.normal_rollover));
    ui.mouse_release(&cmd("SelectLeft"));
    assert_eq!(ui.state(k), Some(st.normal));
}

#[test]
fn state_switch_passes_to_children_and_falls_back_to_zero() {
    let vv = v();
    let c1 = state(z(field(0x2, 0, 0, 5, 5), 0, 1), 0x50, 0, vec![], vec![]);
    let c2 = prop(
        state(z(field(0x3, 0, 0, 5, 5), 0, 2), 0x50, 0, vec![], vec![]),
        vv.pids.ignore_parent_state,
        b(true),
    );
    let p = state(flags(field(0x1, 0, 0, 10, 10), 1), 0x50, 1, vec![], vec![]);
    let p = child(child(p, c1), c2);
    let mut ui = ui_one(0x22FF_000A, 100, 100, vec![p], 100, 100);
    let k = ui.open_panel(Did(0x22FF_000A), 0x1).unwrap();
    let k1 = ui.find_child(k, 0x2).unwrap();
    let k2 = ui.find_child(k, 0x3).unwrap();
    ui.switch_state(k, 0x50);
    assert_eq!(ui.state(k), Some(0x50));
    assert_eq!(ui.state(k1), Some(0x50));
    assert_eq!(ui.state(k2), Some(0));
    // An undefined state becomes 0 and (own flag bit 0) reaches the children as 0.
    ui.switch_state(k, 0x77);
    assert_eq!(ui.state(k), Some(0));
    assert_eq!(ui.state(k1), Some(0));
}

#[test]
fn state_properties_override_own_and_runtime_overrides_state() {
    let vv = v();
    let pid = vv.pids.meter_offset;
    let e = state(
        prop(field(0x1, 0, 0, 10, 10), pid, int(1)),
        0x50,
        0,
        vec![(pid, int(2))],
        vec![],
    );
    let mut ui = ui_one(0x22FF_000B, 100, 100, vec![e], 100, 100);
    let k = ui.open_panel(Did(0x22FF_000B), 0x1).unwrap();
    assert_eq!(ui.property(k, pid), Some(&int(1)));
    ui.switch_state(k, 0x50);
    assert_eq!(ui.property(k, pid), Some(&int(2)));
    ui.set_property(k, pid, int(3));
    assert_eq!(ui.property(k, pid), Some(&int(3)));
    ui.clear_property(k, pid);
    assert_eq!(ui.property(k, pid), Some(&int(2)));
}

#[test]
fn media_player_animation_fade_pause_and_done_message() {
    let vv = v();
    let frames = vec![Did(0x4100_0001), Did(0x4100_0002), Did(0x4100_0003)];
    let e = field(0x1, 0, 0, 10, 10);
    let e = prop(e, vv.pids.notify_on_media_done, b(true));
    let e = media(
        e,
        MediaItem::Animation {
            duration: 1.0,
            draw_mode: DrawMode(4),
            looping: false,
            frames,
        },
    );
    let e = media(e, MediaItem::Pause { min: 0.5, max: 0.5 });
    let e = media(
        e,
        MediaItem::Fade {
            start: 1.0,
            end: 0.0,
            duration: 1.0,
        },
    );
    let e = media(
        e,
        MediaItem::ColouredImage {
            did: Did(0),
            draw_mode: DrawMode(4),
            colour: Argb(0xBF00_0000),
        },
    );
    let mut ui = ui_one(0x22FF_000C, 100, 100, vec![e], 100, 100);
    ui.register_handler(0x1, vv.msgs.media_done);
    let k = ui.open_panel(Did(0x22FF_000C), 0x1).unwrap();
    assert_eq!(ui.base_image(k), Some((ImageRef::Did(0x4100_0001), 4)));
    ui.set_time(0.5);
    ui.frame_work();
    assert_eq!(ui.base_image(k), Some((ImageRef::Did(0x4100_0002), 4)));
    ui.set_time(1.0);
    ui.frame_work();
    assert_eq!(ui.base_image(k), Some((ImageRef::Did(0x4100_0003), 4)));
    // The pause starts at 1.0 and ends after 1.5.
    ui.set_time(1.4);
    ui.frame_work();
    ui.set_time(1.6);
    ui.frame_work();
    // The fade started at 1.6: half way at 2.1.
    ui.set_time(2.1);
    ui.frame_work();
    let o = ui.prop(k, vv.pids.object_opacity).float();
    assert!((o - 0.5).abs() < 1e-4, "{o}");
    assert_eq!(ui.overlay(k), None);
    ui.set_time(3.0);
    ui.frame_work();
    assert_eq!(ui.prop(k, vv.pids.object_opacity).float(), 0.0);
    assert_eq!(ui.overlay(k), Some((None, 0xBF00_0000, 4)));
    let done = ui
        .take_events()
        .into_iter()
        .any(|e| matches!(e, UiEvent::Game { message } if message.msg == vv.msgs.media_done));
    assert!(done);
}

#[test]
fn state_item_waits_for_creation_and_switches() {
    let e = state(
        field(0x1, 0, 0, 10, 10),
        0x51,
        0,
        vec![],
        vec![image_item(0x4100_0051, 2)],
    );
    let e = media(
        e,
        MediaItem::State {
            state: 0x51,
            probability: 1.0,
        },
    );
    let mut ui = ui_one(0x22FF_000D, 100, 100, vec![e], 100, 100);
    let k = ui.open_panel(Did(0x22FF_000D), 0x1).unwrap();
    ui.frame_work();
    assert_eq!(ui.state(k), Some(0x51));
    assert_eq!(ui.base_image(k), Some((ImageRef::Did(0x4100_0051), 2)));
}

#[test]
fn unresolvable_base_is_not_created_and_counted() {
    let good = field(0x1, 0, 0, 10, 10);
    let bad = based(0x2, 0x99, 0x22FF_000E, 0, 0, 10, 10);
    let p = child(child(field(0x3, 0, 0, 50, 50), z(bad, 0, 1)), z(good, 0, 2));
    let mut ui = ui_one(0x22FF_000E, 100, 100, vec![p], 100, 100);
    let k = ui.open_panel(Did(0x22FF_000E), 0x3).unwrap();
    assert_eq!(ui.children(k).len(), 1);
    assert_eq!(
        ui.census()
            .resolve_failures
            .get("Base element missing from the base layout's top level"),
        Some(&1)
    );
}

#[test]
fn unknown_game_type_falls_back_and_engine_type_without_factory_is_dropped() {
    let p = child(
        child(
            field(0x1, 0, 0, 50, 50),
            z(typed(0x2, 0x1000_0777, 0, 0, 5, 5), 0, 1),
        ),
        z(typed(0x3, 13, 0, 0, 5, 5), 0, 2),
    );
    let mut ui = ui_one(0x22FF_000F, 100, 100, vec![p], 100, 100);
    let k = ui.open_panel(Did(0x22FF_000F), 0x1).unwrap();
    assert_eq!(ui.children(k).len(), 1);
    assert_eq!(ui.census().fallback_types.get(&0x1000_0777), Some(&1));
    assert_eq!(ui.census().no_factory.get(&13), Some(&1));
}

#[test]
fn stretch_root_keeps_children_boxes() {
    let vv = v();
    let c = edges(field(0x2, 10, 10, 20, 20), [5, 5, 5, 5]);
    let p = prop(
        child(field(0x1, 0, 0, 100, 100), c),
        vv.pids.object_mode,
        en(1),
    );
    let mut ui = ui_one(0x22FF_0010, 100, 100, vec![p], 100, 100);
    let k = ui.open_panel(Did(0x22FF_0010), 0x1).unwrap();
    let ck = ui.find_child(k, 0x2).unwrap();
    // Stretch: the element resize changes the scale, not the layout box.
    ui.resize(k, 200, 200);
    assert_eq!(ui.layout_box(k).unwrap().w(), 100);
    assert_eq!(ui.layout_box(ck), Some(IBox::from_xywh(10, 10, 20, 20)));
    assert!((ui.node_scale_at_or_above(k).0 - 2.0).abs() < 1e-6);
    // CanvasResize: the layout box takes the size and the child follows its edges.
    ui.set_property(k, vv.pids.object_mode, en(3));
    ui.set_node_scale(k, None);
    ui.resize(k, 200, 200);
    assert_eq!(ui.layout_box(k).unwrap().w(), 200);
    assert_eq!(ui.layout_box(ck), Some(IBox::from_xywh(10, 10, 120, 120)));
}

#[test]
fn screen_resize_relays_root_panels() {
    let p = edges(field(0x1, 824, 668, 200, 100), [4, 4, 5, 5]);
    let mut ui = ui_one(0x22FF_0011, 1024, 768, vec![p], 1024, 768);
    let k = ui.open_panel(Did(0x22FF_0011), 0x1).unwrap();
    assert_eq!(ui.layout_box(k), Some(IBox::from_xywh(824, 668, 200, 100)));
    ui.resize_screen(1920, 1080);
    assert_eq!(ui.layout_box(k), Some(IBox::from_xywh(1720, 980, 200, 100)));
    assert!(ui.take_events().contains(&UiEvent::ScreenResized {
        width: 1920,
        height: 1080
    }));
}

#[test]
fn listener_registration_sets_mouse_visible_until_recomputed() {
    let vv = v();
    let f1 = field(0x2, 0, 0, 10, 10);
    let p = child(
        prop(field(0x1, 0, 0, 50, 50), vv.pids.blocks_clicks, b(true)),
        z(f1, 0, 1),
    );
    let mut ui = ui_one(0x22FF_0012, 100, 100, vec![p], 100, 100);
    // Registered before creation: reset by post-creation (UB-I15, §13.11 item 3).
    ui.register_handler(0x2, vv.msgs.mouse_click);
    let k = ui.open_panel(Did(0x22FF_0012), 0x1).unwrap();
    let fk = ui.find_child(k, 0x2).unwrap();
    assert!(!ui.mouse_visible(fk), "post-creation recomputes the flag");
    // A registration after creation sets it on live elements (§13.11 item 2).
    ui.register_handler(0x2, vv.msgs.mouse_click);
    assert!(ui.mouse_visible(fk));
    // A recomputation replaces it (§13.11 item 4).
    ui.set_property(fk, vv.pids.context_menu, b(false));
    assert!(!ui.mouse_visible(fk));
}

#[test]
fn image_item_with_path_and_clear() {
    let e = media(
        field(0x1, 0, 0, 10, 10),
        MediaItem::Image {
            did: Did(0),
            draw_mode: DrawMode(2),
            path: "raw/en/x.jpg".into(),
        },
    );
    let e = state(e, 0x52, 0, vec![], vec![image_item(0, 8)]);
    let mut ui = ui_one(0x22FF_0013, 100, 100, vec![e], 100, 100);
    let k = ui.open_panel(Did(0x22FF_0013), 0x1).unwrap();
    assert_eq!(
        ui.base_image(k),
        Some((ImageRef::File("raw/en/x.jpg".into()), 2))
    );
    ui.switch_state(k, 0x52);
    assert_eq!(ui.base_image(k), None);
}
