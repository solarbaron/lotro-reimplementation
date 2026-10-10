// SPDX-License-Identifier: GPL-3.0-or-later
//! Item slots (`docs/specs/character-panels.md` §3.3, §4.1, §5, §6), item drag and drop
//! (`ui-behaviour.md` §7), viewports as external-content hosts (`render.md` §6.10), the
//! paperdoll's turning (§3.2) and the creation screens' fallback controls
//! (`character-screens.md` §3–§5) on synthetic trees. All values are invented; types,
//! element IDs, states and properties are the placeholders of `CharVocab::default()`.

mod ui_common;

use tessera_formats::did::Did;
use tessera_formats::ui_layout::{ElementDesc, UiLayout};
use tessera_ui::events::{Message, MsgArg};
use tessera_ui::game::avatar::{AvatarInfo, Viewports};
use tessera_ui::game::creation::{
    Choice, ChoiceParts, Slider, Stepper, SwatchGrid, fill_choices, fill_choices_with,
    select_choice, selected_choice,
};
use tessera_ui::game::examine::ExamTemplate;
use tessera_ui::game::items::{
    DropPlace, ItemSlots, SlotInteraction, SlotItem, SlotPlace, Wear, columns_for_width, slot_bit,
};
use tessera_ui::game::{CharVocab, ICON_LAYER_ID, IconImage};
use tessera_ui::{DragPayload, ElementKey, ImageRef, Ui, UiEvent};
use ui_common::*;

const ITEMS: u32 = 0x22FF_6000;
const SLOT: u32 = 0x1000_0100;
const PANEL: u32 = 0x1000_0200;
const A: u32 = 0x1000_0201;
const B: u32 = 0x1000_0202;
const HEAD: u32 = 0x1000_0203;
const AREA: u32 = 0x1000_0204;
const BAG: u32 = 0x1000_0300;
const DOLL: u32 = 0x1000_0400;
const VP: u32 = 0x1000_0401;
const BL: u32 = 0x1000_0402;
const BR: u32 = 0x1000_0403;
const HOST: u32 = 0x1000_0500;
const PLAIN: u32 = 0x1000_0600;
const CHOICE: u32 = 0x1000_0700;
const C_BACK: u32 = 0x1000_0701;
const C_FRAME: u32 = 0x1000_0702;
const C_ICON: u32 = 0x1000_0703;
const C_NAME: u32 = 0x1000_0704;

fn cv() -> CharVocab {
    CharVocab::default()
}

/// `ItemDisplay_SlottedTemplate`: 35 × 35 with the icon and the overlays of §5.
fn slot_template() -> ElementDesc {
    let c = cv();
    let vv = v();
    let (ids, st) = (c.ids, c.states);
    let icon = typed(ids.icon, c.types.item_icon_display, 0, 0, 32, 32);
    let slotted = child(field(ids.slotted_icon, 3, 3, 32, 32), icon);
    let qty = typed(ids.quantity, vv.types.text, 0, 15, 32, 12);
    let mut broken = field(ids.broken, 3, 3, 32, 32);
    for s in [st.broken, st.almost_broken, st.not_broken] {
        broken = state(broken, s, 0, vec![], vec![]);
    }
    let mut unusable = field(ids.unusable, 3, 3, 32, 32);
    for s in [st.overlay_on, st.overlay_off] {
        unusable = state(unusable, s, 0, vec![], vec![]);
    }
    let cooldown = typed(ids.cooldown, vv.types.meter, 3, 3, 32, 32);
    let rollover = field(ids.rollover, 0, 0, 35, 35);
    let selected = field(ids.selected, 0, 0, 35, 35);
    let mut t = field(SLOT, 0, 0, 35, 35);
    for (i, ch) in [slotted, qty, broken, unusable, cooldown, rollover, selected]
        .into_iter()
        .enumerate()
    {
        t = child(t, z(ch, 0, i as u32 + 1));
    }
    t
}

fn items_layout() -> UiLayout {
    let c = cv();
    let vv = v();
    let at = |id: u32, x: i32, y: i32| based(id, SLOT, ITEMS, x, y, 35, 35);
    let head = prop(at(HEAD, 110, 10), c.pids.container_slot, en(0x2));
    let head = prop(
        head,
        c.pids.empty_override,
        PropertyValue::StringInfo(Box::new(tessera_formats::string_info::StringInfo {
            source: tessera_formats::string_info::StringSource::Literal("Head".into()),
            args: None,
        })),
    );
    let mut panel = field(PANEL, 0, 0, 400, 300);
    for (i, ch) in [
        at(A, 10, 10),
        at(B, 60, 10),
        head,
        field(AREA, 200, 0, 150, 150),
    ]
    .into_iter()
    .enumerate()
    {
        panel = child(panel, z(ch, 0, i as u32 + 1));
    }
    // A bag window with its item list.
    let mut list = typed(
        c.ids.backpack_item_list,
        c.types.inventory_display,
        5,
        25,
        200,
        120,
    );
    list = prop(list, vv.pids.lb_horizontal, b(true));
    list = prop(list, vv.pids.lb_max_columns, int(5));
    let bag = child(field(BAG, 500, 0, 220, 160), list);
    // The paperdoll.
    let mut doll = typed(DOLL, c.types.avatar_info, 0, 400, 300, 400);
    doll = prop(doll, c.pids.avatar_viewport, en(VP));
    doll = prop(doll, c.pids.avatar_left, en(BL));
    doll = prop(doll, c.pids.avatar_right, en(BR));
    doll = prop(doll, c.pids.avatar_drag_rotate, b(true));
    let vp = prop(
        typed(VP, c.types.target_viewport, 50, 0, 200, 300),
        c.pids.viewport_scene,
        did(0x2100_0005),
    );
    doll = child(doll, z(vp, 0, 1));
    doll = child(doll, z(typed(BL, vv.types.button, 0, 350, 30, 30), 0, 2));
    doll = child(doll, z(typed(BR, vv.types.button, 270, 350, 30, 30), 0, 3));
    let host = field(HOST, 800, 400, 210, 40);
    // A slot element without `ItemDisplay_Icon`, with its own designed image.
    let plain = media(field(PLAIN, 0, 700, 35, 35), image_item(0x4100_0001, 4));
    // A choice entry like the race button (§4.1): a Button root 261 × 45 with a full-size
    // background, a frame piece, a 48 × 48 icon and a name text.
    let mut choice = typed(CHOICE, vv.types.button, 0, 0, 261, 45);
    for (i, ch) in [
        field(C_BACK, 0, 0, 261, 45),
        field(C_FRAME, 0, 0, 12, 45),
        field(C_ICON, 4, -2, 48, 48),
        typed(C_NAME, vv.types.text, 60, 12, 150, 20),
    ]
    .into_iter()
    .enumerate()
    {
        choice = child(choice, z(ch, 0, i as u32 + 1));
    }
    layout(
        ITEMS,
        1920,
        1080,
        vec![slot_template(), panel, bag, doll, host, plain, choice],
    )
}

use tessera_formats::property::PropertyValue;

fn ui() -> Ui {
    let mut env = env_with(vec![items_layout()]);
    add_image(&mut env, 0x4100_0001, solid(4, 4, 0xFF11_2233));
    add_image(&mut env, 0x4100_0003, solid(40, 40, 0xFF44_5566));
    let mut ui = ui_with(env, 1920, 1080);
    cv().register_classes(&mut ui);
    ui
}

fn item(id: u64, quantity: i64, fits: u32) -> SlotItem {
    SlotItem {
        item: id,
        icon: Some(IconImage::Composed(ImageRef::Did(0x4100_0001))),
        quantity,
        wear: Wear::Sound,
        unusable: false,
        cooldown: None,
        fits,
    }
}

struct Setup {
    ui: Ui,
    slots: ItemSlots,
    a: ElementKey,
    b: ElementKey,
    head: ElementKey,
}

fn setup() -> Setup {
    let mut ui = ui();
    let panel = ui.open_panel(Did(ITEMS), PANEL).unwrap();
    let mut slots = ItemSlots::new(cv());
    let a = ui.find_descendant(panel, A).unwrap();
    let b = ui.find_descendant(panel, B).unwrap();
    let head = ui.find_descendant(panel, HEAD).unwrap();
    slots.attach(&mut ui, a, SlotPlace::Backpack(0)).unwrap();
    slots.attach(&mut ui, b, SlotPlace::Backpack(1)).unwrap();
    assert_eq!(slots.attach_paperdoll(&mut ui, panel), 1);
    let area = ui.find_descendant(panel, AREA).unwrap();
    slots.attach_area(&mut ui, area, 1);
    Setup {
        ui,
        slots,
        a,
        b,
        head,
    }
}

// ----- §5: the slot display ------------------------------------------------------------

#[test]
fn slot_display_icon_quantity_and_overlays() {
    let Setup {
        mut ui,
        mut slots,
        a,
        head,
        ..
    } = setup();
    let ids = cv().ids;
    let st = cv().states;
    let s = slots.slot(SlotPlace::Backpack(0)).unwrap().clone();
    let (qty, broken, unusable, cooldown, rollover) = (
        s.quantity.unwrap(),
        s.broken.unwrap(),
        s.unusable.unwrap(),
        s.cooldown.unwrap(),
        s.rollover.unwrap(),
    );
    assert_eq!(ui.element_id(s.icon), Some(ids.icon));
    // Empty: overlays hidden, nothing to drag, no examination.
    for k in [qty, broken, unusable, cooldown, rollover, s.off[0]] {
        assert!(!ui.is_visible(k));
    }
    assert!(!ui.examines(a));
    // A stack of 20, damaged, unusable, cooling down.
    let mut it = item(7, 20, 0x2000_0004);
    it.wear = Wear::Damaged;
    it.unusable = true;
    it.cooldown = Some(0.25);
    assert!(slots.set(&mut ui, SlotPlace::Backpack(0), Some(it)));
    assert_eq!(ui.text_of(qty), "20");
    assert!(ui.is_visible(qty));
    assert_eq!(ui.state(broken), Some(st.almost_broken));
    assert!(ui.is_visible(broken));
    assert_eq!(ui.state(unusable), Some(st.overlay_on));
    assert!(ui.is_visible(unusable) && ui.is_visible(cooldown));
    assert_eq!(
        ui.base_image(s.icon).map(|(r, _)| r),
        Some(ImageRef::Did(0x4100_0001))
    );
    assert!(ui.examines(a));
    // An icon given as layers: one child field per layer, replaced on the next set.
    let mut layered = item(7, 1, 0);
    layered.icon = Some(IconImage::Layers(vec![
        ImageRef::Did(0x4100_0001),
        ImageRef::Did(0x4100_0002),
    ]));
    slots.set(&mut ui, SlotPlace::Backpack(0), Some(layered.clone()));
    let layers = |ui: &Ui| {
        ui.children(s.icon)
            .into_iter()
            .filter(|c| ui.element_id(*c) == Some(ICON_LAYER_ID))
            .count()
    };
    assert_eq!(layers(&ui), 2);
    assert_eq!(ui.base_image(s.icon), None);
    slots.set(&mut ui, SlotPlace::Backpack(0), Some(layered));
    assert_eq!(layers(&ui), 2);
    // A single item: no quantity; broken.
    let mut one = item(8, 1, 0);
    one.wear = Wear::Broken;
    slots.set(&mut ui, SlotPlace::Backpack(0), Some(one));
    assert!(!ui.is_visible(qty));
    assert_eq!(ui.state(broken), Some(st.broken));
    assert!(!ui.is_visible(unusable) && !ui.is_visible(cooldown));
    // Sound again.
    slots.set(&mut ui, SlotPlace::Backpack(0), Some(item(8, 1, 0)));
    assert_eq!(ui.state(broken), Some(st.not_broken));
    assert!(!ui.is_visible(broken));
    // The empty paperdoll slot shows its own tooltip text (§3.3).
    assert_eq!(
        slots
            .slot(SlotPlace::Equipment(2))
            .unwrap()
            .empty_tooltip
            .as_deref(),
        Some("Head")
    );
    assert!(ui.mouse_visible(head));
    // Rollover follows the pointer.
    ui.pointer_moved(20, 20);
    slots.update_rollover(&mut ui);
    assert!(ui.is_visible(rollover));
    ui.pointer_moved(390, 290);
    slots.update_rollover(&mut ui);
    assert!(!ui.is_visible(rollover));
    assert!(!slots.set(&mut ui, SlotPlace::Backpack(9), None));
}

// ----- ui-behaviour §7 and character-panels §6: drag and drop ----------------------------

fn drag(ui: &mut Ui, from: (i32, i32), to: (i32, i32)) -> Vec<UiEvent> {
    ui.take_events();
    ui.pointer_moved(from.0, from.1);
    ui.mouse_press(&cmd("SelectLeft"));
    ui.pointer_moved(from.0 + 5, from.1 + 5);
    ui.pointer_moved(to.0, to.1);
    ui.mouse_release(&cmd("SelectLeft"));
    ui.take_events()
}

fn interaction(
    slots: &ItemSlots,
    ui: &Ui,
    events: &[UiEvent],
    split: bool,
) -> Option<SlotInteraction> {
    events.iter().find_map(|e| slots.handle_event(ui, e, split))
}

#[test]
fn drag_between_slots_onto_the_paperdoll_and_out() {
    let Setup {
        mut ui,
        mut slots,
        a,
        ..
    } = setup();
    slots.set(
        &mut ui,
        SlotPlace::Backpack(0),
        Some(item(7, 4, 0x2000_0002)),
    );
    assert_eq!(
        ui.property(a, ui.vocab().pids.dragable),
        Some(&PropertyValue::Bool(true))
    );
    // To another backpack slot, with the split modifier.
    let ev = drag(&mut ui, (27, 27), (77, 27));
    assert!(ev.iter().any(|e| matches!(e, UiEvent::DragBegan { .. })));
    assert_eq!(
        interaction(&slots, &ui, &ev, true),
        Some(SlotInteraction::Dropped {
            item: 7,
            from: SlotPlace::Backpack(0),
            target: DropPlace::Slot(SlotPlace::Backpack(1)),
            split: true,
        })
    );
    // On the paperdoll slot it fits (Head, bit 0x2).
    let ev = drag(&mut ui, (27, 27), (127, 27));
    assert_eq!(
        interaction(&slots, &ui, &ev, false),
        Some(SlotInteraction::Dropped {
            item: 7,
            from: SlotPlace::Backpack(0),
            target: DropPlace::Slot(SlotPlace::Equipment(2)),
            split: false,
        })
    );
    // An item that does not fit the Head slot: the slot refuses (§7.3), nothing happens.
    slots.set(
        &mut ui,
        SlotPlace::Backpack(0),
        Some(item(9, 1, 0x2000_0004)),
    );
    let ev = drag(&mut ui, (27, 27), (127, 27));
    assert_eq!(interaction(&slots, &ui, &ev, false), None);
    assert!(ev.iter().any(|e| matches!(
        e,
        UiEvent::DragEnded {
            accepted: false,
            ..
        }
    )));
    // On the bag area outside the slots.
    let ev = drag(&mut ui, (27, 27), (300, 100));
    assert_eq!(
        interaction(&slots, &ui, &ev, false),
        Some(SlotInteraction::Dropped {
            item: 9,
            from: SlotPlace::Backpack(0),
            target: DropPlace::Area(1),
            split: false,
        })
    );
    // Out of every window: a destroy request.
    let ev = drag(&mut ui, (27, 27), (1500, 900));
    assert_eq!(
        interaction(&slots, &ui, &ev, false),
        Some(SlotInteraction::DestroyRequested {
            item: 9,
            from: SlotPlace::Backpack(0),
        })
    );
    // The payload during a drag, and the drag icon copies the icon.
    ui.pointer_moved(27, 27);
    ui.mouse_press(&cmd("SelectLeft"));
    ui.pointer_moved(40, 40);
    assert_eq!(
        ui.drag_payload(),
        Some(DragPayload {
            item: 9,
            fits: 0x2000_0004
        })
    );
    assert_eq!(ui.drag_source(), Some(a));
    ui.mouse_release(&cmd("SelectLeft"));
    // An empty slot does not drag.
    slots.set(&mut ui, SlotPlace::Backpack(0), None);
    let ev = drag(&mut ui, (27, 27), (77, 27));
    assert!(!ev.iter().any(|e| matches!(e, UiEvent::DragBegan { .. })));
}

#[test]
fn right_click_activates() {
    let Setup {
        mut ui,
        mut slots,
        b,
        ..
    } = setup();
    slots.set(&mut ui, SlotPlace::Backpack(1), Some(item(3, 1, 0)));
    ui.take_events();
    ui.pointer_moved(77, 27);
    ui.mouse_press(&cmd("SelectRight"));
    ui.mouse_release(&cmd("SelectRight"));
    let ev = ui.take_events();
    assert_eq!(
        interaction(&slots, &ui, &ev, false),
        Some(SlotInteraction::Activated {
            item: 3,
            at: SlotPlace::Backpack(1),
        })
    );
    // A left click is no activation.
    ui.mouse_press(&cmd("SelectLeft"));
    ui.mouse_release(&cmd("SelectLeft"));
    let ev = ui.take_events();
    assert_eq!(interaction(&slots, &ui, &ev, false), None);
    assert_eq!(slots.slot_of(&ui, b).unwrap().place, SlotPlace::Backpack(1));
    // Destroyed slots are forgotten.
    let panel = ui.parent(b).unwrap();
    ui.destroy(panel);
    slots.prune(&ui);
    assert!(slots.slots().is_empty());
}

// ----- §4.1: bags -------------------------------------------------------------------------

#[test]
fn bag_list_columns_and_slots() {
    let mut ui = ui();
    let window = ui.open_panel(Did(ITEMS), BAG).unwrap();
    let mut slots = ItemSlots::new(cv());
    let t = ExamTemplate {
        layout: ITEMS,
        element: SLOT,
    };
    let bag = slots
        .attach_bag(&mut ui, window, 2, t, 15..22, Some(3))
        .unwrap();
    assert_eq!(bag.entries.len(), 7);
    assert_eq!(ui.list_grid(bag.list), Some((3, 3)));
    assert_eq!(
        slots.slot_of(&ui, bag.entries[6]).unwrap().place,
        SlotPlace::Backpack(21)
    );
    // Entries are laid out row by row with the column count.
    let b4 = ui.layout_box(bag.entries[4]).unwrap();
    assert_eq!((b4.l, b4.t), (35, 35));
    // Filling again replaces the entries.
    let bag = slots
        .attach_bag(&mut ui, window, 2, t, 15..17, None)
        .unwrap();
    assert_eq!(bag.entries.len(), 2);
    assert_eq!(slots.slots().len(), 2);
    assert_eq!(columns_for_width(180, 35), 5);
    assert_eq!(columns_for_width(10, 35), 1);
    assert_eq!(columns_for_width(10, 0), 1);
    assert_eq!(slot_bit(2), 0x2);
    assert_eq!(slot_bit(0), 0);
    assert_eq!(slot_bit(33), 0);
}

// ----- render.md §6.10 and character-panels §3.2 -------------------------------------------

fn game_msg(ui: &Ui, source: ElementKey, msg: u32) -> UiEvent {
    UiEvent::Game {
        message: Message {
            seq: 0,
            source,
            source_id: ui.element_id(source).unwrap(),
            msg,
            arg: MsgArg::default(),
        },
    }
}

#[test]
fn viewports_host_external_content() {
    let mut ui = ui();
    let doll = ui.open_panel(Did(ITEMS), DOLL).unwrap();
    let mut vps = Viewports::new(cv(), 100);
    let hosts = vps.attach(&mut ui, doll);
    assert_eq!(hosts.len(), 1);
    assert_eq!(hosts[0].id, 100);
    assert_eq!(hosts[0].scene, 0x2100_0005);
    // Attaching again adds nothing.
    assert!(vps.attach(&mut ui, doll).is_empty());
    let t = vps.targets(&ui);
    assert_eq!(t.len(), 1);
    assert_eq!(t[0].size, (200, 300));
    assert_eq!((t[0].displayed.l, t[0].displayed.t), (50, 400));
    assert!(t[0].displayed_now);
    vps.detach(&mut ui, hosts[0].element);
    assert!(vps.targets(&ui).is_empty());
}

#[test]
fn paperdoll_turns_by_buttons_and_drag() {
    let mut ui = ui();
    let doll = ui.open_panel(Did(ITEMS), DOLL).unwrap();
    let mut av = AvatarInfo::attach(&mut ui, &cv(), doll);
    let (l, r, vp) = (av.left.unwrap(), av.right.unwrap(), av.viewport.unwrap());
    assert!(av.drag_rotate);
    let hot = ui.vocab().msgs.button_hot_click;
    assert!(av.handle_event(&ui, &game_msg(&ui, r, hot)));
    assert_eq!(av.yaw, 15.0);
    av.handle_event(&ui, &game_msg(&ui, l, hot));
    av.handle_event(&ui, &game_msg(&ui, l, hot));
    assert_eq!(av.yaw, 345.0);
    // Drag in the viewport: 0.5° per pixel.
    ui.take_events();
    ui.pointer_moved(150, 500);
    ui.mouse_press(&cmd("SelectLeft"));
    let started = ui.take_events().iter().any(|e| av.handle_event(&ui, e));
    assert!(started && av.dragging());
    assert_eq!(ui.captured(), Some(vp));
    ui.pointer_moved(170, 500);
    assert!(av.update(&ui));
    assert_eq!(av.yaw, 355.0);
    ui.mouse_release(&cmd("SelectLeft"));
    assert!(!av.update(&ui));
    assert!(!av.dragging());
}

// ----- character-screens §3–§5: fallback controls ----------------------------------------

#[test]
fn stepper_wraps_and_counts() {
    let mut ui = ui();
    let host = ui.open_panel(Did(ITEMS), HOST).unwrap();
    let mut s = Stepper::build(&mut ui, host, "Hair", 3, 0).unwrap();
    assert_eq!(ui.text_of(s.value.unwrap()), "1 / 3");
    assert_eq!(ui.text_of(s.label.unwrap()), "Hair");
    // A real click on the previous arrow (a code-made Button).
    let pb = ui.displayed_box(s.prev);
    ui.take_events();
    ui.pointer_moved(pb.l + 2, pb.t + 2);
    ui.mouse_press(&cmd("SelectLeft"));
    ui.mouse_release(&cmd("SelectLeft"));
    let ev = ui.take_events();
    let changed: Vec<usize> = ev
        .iter()
        .filter_map(|e| s.handle_event(&mut ui, e))
        .collect();
    assert_eq!(changed, vec![2]);
    assert_eq!(ui.text_of(s.value.unwrap()), "3 / 3");
    let click = ui.vocab().msgs.button_click;
    let next = game_msg(&ui, s.next, click);
    assert_eq!(s.handle_event(&mut ui, &next), Some(0));
    s.set_value_text(&mut ui, Some("Preset A".into()));
    assert_eq!(ui.text_of(s.value.unwrap()), "Preset A");
    s.set(&mut ui, 9, 0);
    assert_eq!(s.handle_event(&mut ui, &next), None);
    s.release(&mut ui);
}

#[test]
fn slider_follows_presses_and_drags() {
    let mut ui = ui();
    let host = ui.open_panel(Did(ITEMS), HOST).unwrap();
    // 210 wide: the thumb travels 200 pixels.
    let mut s = Slider::build(&mut ui, host, 0.8, 1.2, 1.0).unwrap();
    assert_eq!(ui.layout_box(s.thumb).unwrap().l, 100);
    let tb = ui.displayed_box(s.track);
    ui.take_events();
    ui.pointer_moved(tb.l + 5 + 50, tb.t + 2);
    ui.mouse_press(&cmd("SelectLeft"));
    let ev = ui.take_events();
    let v: Vec<f32> = ev
        .iter()
        .filter_map(|e| s.handle_event(&mut ui, e))
        .collect();
    assert_eq!(v.len(), 1);
    assert!((s.value() - 0.9).abs() < 1e-5, "{}", s.value());
    ui.pointer_moved(tb.l + 5 + 200, tb.t + 2);
    assert!(s.update(&mut ui).is_some());
    assert!((s.value() - 1.2).abs() < 1e-5);
    ui.mouse_release(&cmd("SelectLeft"));
    assert_eq!(s.update(&mut ui), None);
    // Hostile bounds.
    let mut odd = Slider::build(&mut ui, host, f32::NAN, -3.0, f32::INFINITY).unwrap();
    assert_eq!(odd.range(), (-3.0, 0.0));
    odd.set_value(&mut ui, f32::NAN);
    assert_eq!(odd.value(), -3.0);
}

#[test]
fn swatches_pick_palette_pixels() {
    let mut ui = ui();
    let host = ui.open_panel(Did(ITEMS), HOST).unwrap();
    let mut g = SwatchGrid::build(&mut ui, host, ImageRef::Did(0x4100_0001), (8, 8)).unwrap();
    assert_eq!(ui.layout_box(g.field).unwrap().w(), 4);
    let click = MsgArg {
        command: Some(cmd("SelectLeft")),
        pos: (2, 9),
        ..MsgArg::default()
    };
    let ev = UiEvent::Game {
        message: Message {
            seq: 0,
            source: g.field,
            source_id: ui.element_id(g.field).unwrap(),
            msg: ui.vocab().msgs.mouse_click,
            arg: click,
        },
    };
    assert_eq!(g.handle_event(&mut ui, &ev), Some((2, 3)));
    assert_eq!(g.colour(&mut ui), Some(0xFF11_2233));
    assert!(ui.is_visible(g.marker.unwrap()));
}

#[test]
fn choice_lists_fill_and_select() {
    let mut ui = ui();
    let window = ui.open_panel(Did(ITEMS), BAG).unwrap();
    let list = ui
        .find_descendant(window, cv().ids.backpack_item_list)
        .unwrap();
    let t = ExamTemplate {
        layout: ITEMS,
        element: SLOT,
    };
    let choices: Vec<Choice> = ["Race A", "Race B", "Race C"]
        .iter()
        .map(|s| Choice {
            label: (*s).into(),
            icon: Some(ImageRef::Did(0x4100_0001)),
            disabled: false,
        })
        .collect();
    let entries = fill_choices(&mut ui, list, t, &choices);
    assert_eq!(entries.len(), 3);
    assert!(entries.iter().all(Option::is_some));
    assert_eq!(
        ui.text_of(
            ui.find_descendant(entries[1].unwrap(), cv().ids.quantity)
                .unwrap()
        ),
        "Race B"
    );
    select_choice(&mut ui, list, &entries, 2);
    assert_eq!(selected_choice(&ui, list, &entries), Some(2));
    // Filling again replaces the entries.
    let again = fill_choices(&mut ui, list, t, &choices[..1]);
    assert_eq!(ui.list_entries(list).len(), 1);
    assert_eq!(selected_choice(&ui, list, &again), None);
}

// ----- review regressions ----------------------------------------------------------------

#[test]
fn slot_without_an_icon_child_restores_its_own_image_when_emptied() {
    // §5: an empty slot shows no item icon; a slot element without `ItemDisplay_Icon`
    // shows the item's icon on itself and gets its own image back.
    let mut ui = ui();
    let plain = ui.open_panel(Did(ITEMS), PLAIN).unwrap();
    let designed = ui.base_image(plain);
    assert_eq!(
        designed.as_ref().map(|(r, _)| r),
        Some(&ImageRef::Did(0x4100_0001))
    );
    let mut slots = ItemSlots::new(cv());
    let place = SlotPlace::Other {
        container: 9,
        index: 0,
    };
    slots.attach(&mut ui, plain, place).unwrap();
    assert_eq!(slots.slot(place).unwrap().icon, plain);
    assert_eq!(
        ui.base_image(plain),
        designed,
        "attaching keeps the slot's image"
    );
    let mut it = item(1, 1, 0);
    it.icon = Some(IconImage::Composed(ImageRef::Did(0x4100_0003)));
    slots.set(&mut ui, place, Some(it.clone()));
    assert_eq!(
        ui.base_image(plain).map(|(r, _)| r),
        Some(ImageRef::Did(0x4100_0003))
    );
    slots.set(&mut ui, place, None);
    assert_eq!(
        ui.base_image(plain),
        designed,
        "emptied: the item's icon is gone"
    );
    // An item without an icon after one with an icon: the slot's own image too.
    slots.set(&mut ui, place, Some(it));
    let mut bare = item(2, 1, 0);
    bare.icon = None;
    slots.set(&mut ui, place, Some(bare));
    assert_eq!(ui.base_image(plain), designed);
    // Layers: their fields go when the slot empties.
    let mut layered = item(3, 1, 0);
    layered.icon = Some(IconImage::Layers(vec![
        ImageRef::Did(0x4100_0001),
        ImageRef::Did(0x4100_0003),
    ]));
    slots.set(&mut ui, place, Some(layered));
    let layers = |ui: &Ui| {
        ui.children(plain)
            .into_iter()
            .filter(|c| ui.element_id(*c) == Some(ICON_LAYER_ID))
            .count()
    };
    assert_eq!(layers(&ui), 2);
    slots.set(&mut ui, place, None);
    assert_eq!(layers(&ui), 0);
    assert_eq!(ui.base_image(plain), designed);
    // Attaching again keeps the designed image as the one to restore.
    slots.set(&mut ui, place, Some(item(4, 1, 0)));
    slots.attach(&mut ui, plain, place).unwrap();
    assert_eq!(ui.base_image(plain), designed);
}

#[test]
fn icon_layers_are_drawn_once_at_their_own_size() {
    // icon-layers.md §3.4: no layer is scaled or repeated; a field per layer of the
    // image's own size within the icon (32 × 32 here), a missing image the icon's size.
    let Setup {
        mut ui, mut slots, ..
    } = setup();
    let icon = slots.slot(SlotPlace::Backpack(0)).unwrap().icon;
    let mut layered = item(7, 1, 0);
    layered.icon = Some(IconImage::Layers(vec![
        ImageRef::Did(0x4100_0003),
        ImageRef::Did(0x4100_0001),
        ImageRef::Did(0x4100_00FF),
    ]));
    slots.set(&mut ui, SlotPlace::Backpack(0), Some(layered));
    let sizes = |ui: &Ui| -> Vec<(i32, i32, i32, i32)> {
        ui.children(icon)
            .into_iter()
            .filter(|c| ui.element_id(*c) == Some(ICON_LAYER_ID))
            .map(|c| {
                let b = ui.layout_box(c).unwrap();
                (b.l, b.t, b.w(), b.h())
            })
            .collect()
    };
    assert_eq!(
        sizes(&ui),
        vec![(0, 0, 32, 32), (0, 0, 4, 4), (0, 0, 32, 32)]
    );
    // Fitting again keeps the sizes of loaded images.
    slots.fit_icons(&mut ui);
    assert_eq!(
        sizes(&ui),
        vec![(0, 0, 32, 32), (0, 0, 4, 4), (0, 0, 32, 32)]
    );
}

#[test]
fn rollover_turns_off_after_prune_and_reattach() {
    let Setup {
        mut ui,
        mut slots,
        a,
        head,
        ..
    } = setup();
    let over = slots
        .slot(SlotPlace::Equipment(2))
        .unwrap()
        .rollover
        .unwrap();
    ui.pointer_moved(127, 27);
    slots.update_rollover(&mut ui);
    assert!(ui.is_visible(over));
    // Another slot goes away: the list shrinks, the overlay still turns off.
    ui.destroy(a);
    slots.prune(&ui);
    ui.pointer_moved(390, 290);
    slots.update_rollover(&mut ui);
    assert!(!ui.is_visible(over), "head rollover stuck after prune");
    // Re-attaching other slots changes nothing for the hovered one.
    ui.pointer_moved(127, 27);
    slots.update_rollover(&mut ui);
    assert!(ui.is_visible(over));
    let b = slots.slot(SlotPlace::Backpack(1)).unwrap().element;
    slots.attach(&mut ui, b, SlotPlace::Backpack(5)).unwrap();
    ui.pointer_moved(390, 290);
    slots.update_rollover(&mut ui);
    assert!(!ui.is_visible(over));
    assert!(ui.exists(head));
}

#[test]
fn drop_on_the_bag_window_outside_the_list_is_the_bag_area() {
    // character-panels.md §6: a drop on the bag window outside a slot.
    let Setup {
        mut ui, mut slots, ..
    } = setup();
    let window = ui.open_panel(Did(ITEMS), BAG).unwrap();
    let t = ExamTemplate {
        layout: ITEMS,
        element: SLOT,
    };
    slots
        .attach_bag(&mut ui, window, 4, t, 10..12, None)
        .unwrap();
    slots.set(&mut ui, SlotPlace::Backpack(0), Some(item(7, 1, 0)));
    // The window's title strip, above the item list (list at y 25).
    let ev = drag(&mut ui, (27, 27), (600, 10));
    assert_eq!(
        interaction(&slots, &ui, &ev, false),
        Some(SlotInteraction::Dropped {
            item: 7,
            from: SlotPlace::Backpack(0),
            target: DropPlace::Area(4),
            split: false,
        })
    );
    // A slot of the bag is still a slot.
    let ev = drag(&mut ui, (27, 27), (520, 40));
    assert_eq!(
        interaction(&slots, &ui, &ev, false),
        Some(SlotInteraction::Dropped {
            item: 7,
            from: SlotPlace::Backpack(0),
            target: DropPlace::Slot(SlotPlace::Backpack(10)),
            split: false,
        })
    );
}

#[test]
fn large_bags_fill_in_one_layout() {
    let mut ui = ui();
    let window = ui.open_panel(Did(ITEMS), BAG).unwrap();
    let mut slots = ItemSlots::new(cv());
    let t = ExamTemplate {
        layout: ITEMS,
        element: SLOT,
    };
    // Capped at MAX_SLOTS.
    let bag = slots
        .attach_bag(&mut ui, window, 6, t, 75..10_000, Some(5))
        .unwrap();
    assert_eq!(bag.entries.len(), tessera_ui::game::items::MAX_SLOTS);
    assert_eq!(ui.list_entries(bag.list), bag.entries);
    assert_eq!(slots.slots().len(), tessera_ui::game::items::MAX_SLOTS);
    let last = *bag.entries.last().unwrap();
    assert_eq!(
        slots.slot_of(&ui, last).unwrap().place,
        SlotPlace::Backpack(75 + 4095)
    );
    let b = ui.layout_box(last).unwrap();
    // Slot 4095 of five columns: column 0, row 819.
    assert_eq!((b.l, b.t), (0, 35 * 819));
    // Refilling replaces every entry and slot.
    let before = ui.element_count();
    let again = slots
        .attach_bag(&mut ui, window, 6, t, 75..80, None)
        .unwrap();
    assert_eq!(again.entries.len(), 5);
    assert_eq!(slots.slots().len(), 5);
    assert!(ui.element_count() < before);
}

#[test]
fn choice_entries_show_the_label_and_icon_in_their_parts() {
    // character-screens.md §4.1: the entry is a Button with a background, a frame piece,
    // the icon and the name; the label goes to the name, not the Button, and the icon to
    // the icon part, not the background or the frame.
    let mut ui = ui();
    let window = ui.open_panel(Did(ITEMS), BAG).unwrap();
    let list = ui
        .find_descendant(window, cv().ids.backpack_item_list)
        .unwrap();
    let t = ExamTemplate {
        layout: ITEMS,
        element: CHOICE,
    };
    let choices = vec![Choice {
        label: "Race A".into(),
        icon: Some(ImageRef::Did(0x4100_0001)),
        disabled: false,
    }];
    let check = |ui: &mut Ui, e: ElementKey| {
        let name = ui.find_descendant(e, C_NAME).unwrap();
        assert_eq!(ui.text_of(name), "Race A");
        assert_eq!(ui.text_of(e), "", "the entry's own Button text stays");
        let icon = ui.find_descendant(e, C_ICON).unwrap();
        assert_eq!(
            ui.base_image(icon).map(|(r, _)| r),
            Some(ImageRef::Did(0x4100_0001))
        );
        for part in [C_BACK, C_FRAME] {
            assert_eq!(ui.base_image(ui.find_descendant(e, part).unwrap()), None);
        }
    };
    // The fallback rule.
    let entries = fill_choices(&mut ui, list, t, &choices);
    check(&mut ui, entries[0].unwrap());
    // Named parts.
    let parts = ChoiceParts {
        name: Some(C_NAME),
        icon: Some(C_ICON),
    };
    let entries = fill_choices_with(&mut ui, list, t, parts, &choices);
    check(&mut ui, entries[0].unwrap());
    assert_eq!(ui.list_entries(list).len(), 1);
    // The parts of the three lists resolve from the vocabulary.
    let ids = cv().ids;
    assert_eq!(ChoiceParts::race(&ids).name, Some(ids.race_name));
    assert_eq!(ChoiceParts::class(&ids).icon, Some(ids.class_icon));
    assert_eq!(ChoiceParts::character(&ids).icon, Some(ids.character_class));
}
