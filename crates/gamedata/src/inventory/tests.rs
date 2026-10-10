// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the inventory model, one or more per rule.

use std::collections::BTreeSet;

use tessera_formats::ui_layout::{ElementDesc, Rect, UiLayout};
use tessera_formats::{Did, PropertyArray, PropertyEntry, PropertySet, PropertyValue};

use super::*;

const BAGS: BagLayout = BagLayout {
    base_slots: 45,
    slots_per_bag: 15,
    bags: 6,
};

fn rec(d: u32) -> ItemRef {
    ItemRef::Record(Did(d))
}

fn add(inv: &mut Inventory, item: ItemId, at: ItemPlace, quantity: u32) -> Vec<InventoryChange> {
    inv.apply(&InventoryEvent::ItemAdded {
        item,
        what: rec(0x7000_0000 + u32::try_from(item).unwrap_or(0)),
        at,
        quantity,
    })
}

#[test]
fn slot_bits_and_keys() {
    assert_eq!(slot_bit(1), Some(1));
    assert_eq!(slot_bit(2), Some(2));
    assert_eq!(slot_bit(64), Some(1 << 63));
    assert_eq!(slot_bit(0), None);
    assert_eq!(slot_bit(65), None);
    assert_eq!(slot_keys(0x2 | 0x4000).collect::<Vec<_>>(), vec![2, 15]);
    assert_eq!(single_slot_key(0x2000), Some(14));
    assert_eq!(single_slot_key(0x6), None);
    assert_eq!(single_slot_key(0), None);
    assert!(has_slot_key(0x10000, 17));
    assert!(!has_slot_key(0x10000, 0));
}

#[test]
fn api_equipment_mapping() {
    assert_eq!(api_equipment_index(2), Some(1));
    assert_eq!(api_equipment_index(3), Some(2));
    assert_eq!(api_equipment_index(22), Some(21));
    assert_eq!(api_equipment_index(1), None);
    assert_eq!(
        api_equipment_index(23),
        None,
        "FX slots are invisible to plugins"
    );
    assert_eq!(key_of_api_equipment_index(2.0), Some(3));
    assert_eq!(key_of_api_equipment_index(21.0), Some(22));
    for bad in [0.0, 22.0, 1.5, -1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(key_of_api_equipment_index(bad), None, "{bad}");
    }
    for key in 2..=22 {
        let v = api_equipment_index(key).unwrap_or(0);
        assert_eq!(key_of_api_equipment_index(f64::from(v)), Some(key));
    }
}

#[test]
fn backpack_index_mapping() {
    assert_eq!(backpack_index(0), 1);
    assert_eq!(slot_of_backpack_index(1.0, 30), Some(0));
    assert_eq!(slot_of_backpack_index(30.0, 30), Some(29));
    for bad in [0.0, 31.0, 1.5, f64::NAN, -3.0, 1e300] {
        assert_eq!(slot_of_backpack_index(bad, 30), None, "{bad}");
    }
    assert_eq!(slot_of_backpack_index(65_536.0, 65_536), Some(65_535));
}

#[test]
fn cpi3_api_names() {
    let names = |key: u32| -> Option<String> {
        let n = match key {
            2 => "Head",
            3 => "Chest",
            4 => "Legs",
            5 => "Gloves",
            6 => "Boots",
            7 => "Shoulder",
            8 => "Back",
            9 => "Bracelet1",
            10 => "Bracelet2",
            11 => "Necklace",
            12 => "Ring1",
            13 => "Ring2",
            14 => "Earring1",
            15 => "Earring2",
            16 => "Pocket1",
            17 => "Weapon_Primary",
            18 => "Weapon_Secondary",
            19 => "Weapon_Ranged",
            20 => "CraftTool",
            21 => "Class",
            22 => "Mounted",
            _ => return None,
        };
        Some(n.to_owned())
    };
    assert!(check_api_names(names).is_empty());
    let broken = |key: u32| (key != 5).then(|| names(key)).flatten();
    assert_eq!(check_api_names(broken), vec![(4, None)]);
}

#[test]
fn bag_ranges_rule_4_3() {
    // Rule 2: bags 1–5 by range; rule 3: bag 6 from 75 up; rule 4: empty bags not offered.
    assert_eq!(
        BAGS.offered_bags(45),
        vec![(1, 0..15), (2, 15..30), (3, 30..45)]
    );
    assert_eq!(BAGS.bag_range(4, 45), None);
    assert_eq!(BAGS.bag_range(3, 40), Some(30..40));
    assert_eq!(BAGS.bag_range(6, 120), Some(75..120));
    assert_eq!(BAGS.bag_range(5, 120), Some(60..75));
    assert_eq!(BAGS.bag_range(6, 75), None);
    assert_eq!(BAGS.bag_range(0, 75), None);
    assert_eq!(BAGS.bag_range(7, 200), None);
    assert_eq!(BAGS.bag_of_slot(0, 45), Some(1));
    assert_eq!(BAGS.bag_of_slot(44, 45), Some(3));
    assert_eq!(BAGS.bag_of_slot(45, 45), None);
    assert_eq!(BAGS.bag_of_slot(99, 120), Some(6));
    assert_eq!(
        BAGS.bag_of_slot(90, 100),
        Some(6),
        "slots from 75 up are in bag 6"
    );
    // Degenerate layouts never panic.
    let none = BagLayout {
        base_slots: 0,
        slots_per_bag: 0,
        bags: 0,
    };
    assert!(none.offered_bags(100).is_empty());
    assert_eq!(none.bag_of_slot(3, 100), None);
    let one = BagLayout {
        base_slots: 0,
        slots_per_bag: 0,
        bags: 1,
    };
    assert_eq!(one.offered_bags(10), vec![(1, 0..10)]);
    assert_eq!(one.bag_of_slot(3, 10), Some(1));
}

#[test]
fn bag_layout_from_control() {
    let c = InventoryControl {
        base_backpack_slots: Some(45),
        slots_per_bag: Some(15),
        max_bags: Some(6),
        slots_per_pouch: Some(5),
        equipment_slots: Some(21),
    };
    assert_eq!(BagLayout::from_control(&c), Some(BAGS));
    let partial = InventoryControl {
        max_bags: None,
        ..c
    };
    assert_eq!(BagLayout::from_control(&partial), None);
}

#[test]
fn backpack_size_rule_1_and_check_66() {
    // A `bag 30` then items at slots 0, 4 and 29.
    let mut inv = Inventory::new(BAGS);
    assert_eq!(inv.backpack_size(), 45);
    let ev = inv.apply_for_plugins(&InventoryEvent::ContainerSize {
        container: Container::Backpack,
        slots: 30,
    });
    assert_eq!(ev, vec![PluginEvent::BackpackSizeChanged]);
    let mut seen = Vec::new();
    for (id, slot) in [(1, 0), (2, 4), (3, 29)] {
        let c = add(&mut inv, id, ItemPlace::backpack(slot), 1);
        seen.extend(plugin_events(&c));
    }
    assert_eq!(
        seen,
        vec![
            PluginEvent::BackpackItemAdded { index: 1, item: 1 },
            PluginEvent::BackpackItemAdded { index: 5, item: 2 },
            PluginEvent::BackpackItemAdded { index: 30, item: 3 },
        ]
    );
    assert_eq!(inv.backpack_size(), 30);
    assert_eq!(inv.backpack_item_at_index(1.0).map(|i| i.id), Some(1));
    assert_eq!(inv.backpack_item_at_index(5.0).map(|i| i.id), Some(2));
    assert_eq!(inv.backpack_item_at_index(30.0).map(|i| i.id), Some(3));
    for nil in [2.0, 0.0, 31.0, 1.5] {
        assert!(inv.backpack_item_at_index(nil).is_none(), "{nil}");
    }
    // After a size is set, an arrival never grows N.
    let c = add(&mut inv, 4, ItemPlace::backpack(40), 1);
    assert!(
        !c.iter()
            .any(|c| matches!(c, InventoryChange::BackpackSize { .. }))
    );
    assert_eq!(inv.backpack_size(), 30);
}

#[test]
fn growth_before_first_size_fires_size_changed_first() {
    let mut inv = Inventory::new(BAGS);
    let ev = plugin_events(&add(&mut inv, 9, ItemPlace::backpack(45 + 5), 1));
    assert_eq!(
        ev,
        vec![
            PluginEvent::BackpackSizeChanged,
            PluginEvent::BackpackItemAdded { index: 51, item: 9 },
        ]
    );
    assert_eq!(inv.backpack_size(), 51);
    // No growth for a slot inside N, and N never shrinks without a ContainerSize.
    let ev = plugin_events(&add(&mut inv, 10, ItemPlace::backpack(3), 1));
    assert_eq!(
        ev,
        vec![PluginEvent::BackpackItemAdded { index: 4, item: 10 }]
    );
    inv.apply(&InventoryEvent::ItemRemoved { item: 9 });
    assert_eq!(inv.backpack_size(), 51);
    // A move into the backpack beyond N grows it too.
    add(&mut inv, 11, ItemPlace::equipment(3), 1);
    let ev = inv.apply_for_plugins(&InventoryEvent::ItemMoved {
        item: 11,
        at: ItemPlace::backpack(60),
    });
    assert_eq!(
        ev,
        vec![
            PluginEvent::BackpackSizeChanged,
            PluginEvent::ItemUnequipped { index: 2, item: 11 },
            PluginEvent::BackpackItemAdded {
                index: 61,
                item: 11
            },
        ]
    );
    assert_eq!(inv.backpack_size(), 61);
}

#[test]
fn check_67_moves_destroy_and_sizes() {
    let mut inv = Inventory::new(BAGS);
    inv.apply(&InventoryEvent::ContainerSize {
        container: Container::Backpack,
        slots: 30,
    });
    add(&mut inv, 2, ItemPlace::backpack(4), 1);
    let ev = inv.apply_for_plugins(&InventoryEvent::ItemMoved {
        item: 2,
        at: ItemPlace::backpack(10),
    });
    assert_eq!(
        ev,
        vec![PluginEvent::BackpackItemMoved {
            old_index: 5,
            new_index: 11,
            item: 2
        }]
    );
    let ev = inv.apply_for_plugins(&InventoryEvent::ItemRemoved { item: 2 });
    assert_eq!(
        ev,
        vec![PluginEvent::BackpackItemRemoved { index: 11, item: 2 }]
    );
    let ev = inv.apply_for_plugins(&InventoryEvent::ContainerSize {
        container: Container::Backpack,
        slots: 40,
    });
    assert_eq!(ev, vec![PluginEvent::BackpackSizeChanged]);
    assert_eq!(inv.backpack_size(), 40);
    let again = inv.apply(&InventoryEvent::ContainerSize {
        container: Container::Backpack,
        slots: 40,
    });
    assert!(again.is_empty(), "a second equal size changes nothing");
    assert!(plugin_events(&again).is_empty());
}

#[test]
fn check_68_equipment_view() {
    let mut inv = Inventory::new(BAGS);
    add(&mut inv, 5, ItemPlace::backpack(0), 1);
    let ev = inv.apply_for_plugins(&InventoryEvent::ItemMoved {
        item: 5,
        at: ItemPlace::equipment(3),
    });
    assert_eq!(
        ev,
        vec![
            PluginEvent::BackpackItemRemoved { index: 1, item: 5 },
            PluginEvent::ItemEquipped { index: 2, item: 5 },
        ]
    );
    assert_eq!(inv.equipment_item_at_index(2.0).map(|i| i.id), Some(5));
    assert!(inv.equipment_item_at_index(1.0).is_none());
    assert_eq!(API_EQUIPMENT_SIZE, 21);
    let ev = inv.apply_for_plugins(&InventoryEvent::ItemMoved {
        item: 5,
        at: ItemPlace::backpack(0),
    });
    assert_eq!(
        ev,
        vec![
            PluginEvent::ItemUnequipped { index: 2, item: 5 },
            PluginEvent::BackpackItemAdded { index: 1, item: 5 },
        ]
    );
}

#[test]
fn fx_slots_and_other_containers_are_silent() {
    let mut inv = Inventory::new(BAGS);
    let ev = plugin_events(&add(&mut inv, 1, ItemPlace::equipment(23), 1));
    assert!(ev.is_empty(), "FX slot 23 has no API value");
    let ev = plugin_events(&add(&mut inv, 2, ItemPlace::equipment(1), 1));
    assert!(ev.is_empty(), "key 1 has no API value");
    let vault = ItemPlace {
        container: Container::Vault,
        slot: 3,
    };
    let ev = plugin_events(&add(&mut inv, 3, vault, 1));
    assert!(ev.is_empty());
    let ev = inv.apply_for_plugins(&InventoryEvent::ItemMoved {
        item: 3,
        at: ItemPlace::backpack(2),
    });
    assert_eq!(
        ev,
        vec![PluginEvent::BackpackItemAdded { index: 3, item: 3 }]
    );
    // A move within the equipment between API keys: unequip, then equip.
    add(&mut inv, 4, ItemPlace::equipment(17), 1);
    let ev = inv.apply_for_plugins(&InventoryEvent::ItemMoved {
        item: 4,
        at: ItemPlace::equipment(18),
    });
    assert_eq!(
        ev,
        vec![
            PluginEvent::ItemUnequipped { index: 16, item: 4 },
            PluginEvent::ItemEquipped { index: 17, item: 4 },
        ]
    );
}

#[test]
fn swap_keeps_both_items_visible() {
    // MoveItem onto an occupied slot: the feed moves the item, then the occupant.
    let mut inv = Inventory::new(BAGS);
    add(&mut inv, 1, ItemPlace::backpack(1), 1);
    add(&mut inv, 2, ItemPlace::backpack(2), 1);
    let first = inv.apply_for_plugins(&InventoryEvent::ItemMoved {
        item: 1,
        at: ItemPlace::backpack(2),
    });
    assert_eq!(inv.at(ItemPlace::backpack(2)).map(|i| i.id), Some(1));
    assert!(inv.at(ItemPlace::backpack(1)).is_none());
    let second = inv.apply_for_plugins(&InventoryEvent::ItemMoved {
        item: 2,
        at: ItemPlace::backpack(1),
    });
    assert_eq!(inv.at(ItemPlace::backpack(1)).map(|i| i.id), Some(2));
    assert_eq!(inv.at(ItemPlace::backpack(2)).map(|i| i.id), Some(1));
    assert_eq!(
        [first, second].concat(),
        vec![
            PluginEvent::BackpackItemMoved {
                old_index: 2,
                new_index: 3,
                item: 1
            },
            PluginEvent::BackpackItemMoved {
                old_index: 3,
                new_index: 2,
                item: 2
            },
        ]
    );
    let ids: Vec<ItemId> = inv
        .in_container(Container::Backpack)
        .map(|i| i.id)
        .collect();
    assert_eq!(ids, vec![2, 1]);
    // A shadowed item becomes visible again when the one over it leaves.
    let mut inv = Inventory::new(BAGS);
    add(&mut inv, 1, ItemPlace::backpack(5), 1);
    add(&mut inv, 2, ItemPlace::backpack(6), 1);
    inv.apply(&InventoryEvent::ItemMoved {
        item: 2,
        at: ItemPlace::backpack(5),
    });
    assert_eq!(inv.at(ItemPlace::backpack(5)).map(|i| i.id), Some(2));
    inv.apply(&InventoryEvent::ItemRemoved { item: 2 });
    assert_eq!(inv.at(ItemPlace::backpack(5)).map(|i| i.id), Some(1));
    assert_eq!(inv.in_container(Container::Backpack).count(), 1);
}

#[test]
fn quantity_condition_and_hostile_events() {
    let mut inv = Inventory::new(BAGS);
    add(&mut inv, 1, ItemPlace::backpack(0), 5);
    let ev = inv.apply_for_plugins(&InventoryEvent::ItemQuantity {
        item: 1,
        quantity: 3,
    });
    assert_eq!(ev, vec![PluginEvent::QuantityChanged { item: 1 }]);
    let ev = inv.apply_for_plugins(&InventoryEvent::ItemQuantity {
        item: 1,
        quantity: 3,
    });
    assert!(ev.is_empty(), "no event when the quantity stays the same");
    assert_eq!(inv.item(1).map(|i| i.quantity), Some(3));
    let c = inv.apply(&InventoryEvent::ItemCondition {
        item: 1,
        current: 4,
        max: 10,
    });
    assert_eq!(c.len(), 1);
    assert_eq!(inv.item(1).and_then(|i| i.condition), Some((4, 10)));
    // Unknown items and duplicate additions change nothing.
    for e in [
        InventoryEvent::ItemMoved {
            item: 99,
            at: ItemPlace::backpack(1),
        },
        InventoryEvent::ItemQuantity {
            item: 99,
            quantity: 1,
        },
        InventoryEvent::ItemRemoved { item: 99 },
        InventoryEvent::ItemCondition {
            item: 99,
            current: 1,
            max: 1,
        },
        InventoryEvent::ItemAdded {
            item: 1,
            what: ItemRef::Literal("x".into()),
            at: ItemPlace::backpack(9),
            quantity: 1,
        },
    ] {
        assert!(inv.apply(&e).is_empty(), "{e:?}");
    }
    assert_eq!(inv.len(), 1);
    assert_eq!(inv.item(1).map(|i| i.place), Some(ItemPlace::backpack(0)));
}

#[test]
fn other_container_sizes_and_worn_list() {
    let mut inv = Inventory::new(BAGS);
    let c = inv.apply(&InventoryEvent::ContainerSize {
        container: Container::Vault,
        slots: 10,
    });
    assert_eq!(
        c,
        vec![InventoryChange::ContainerSize {
            container: Container::Vault,
            old: None,
            new: 10
        }]
    );
    assert_eq!(inv.container_size(Container::Vault), Some(10));
    assert_eq!(
        inv.backpack_size(),
        45,
        "a vault size is not the backpack's"
    );
    assert!(plugin_events(&c).is_empty());
    add(&mut inv, 1, ItemPlace::equipment(17), 1);
    add(&mut inv, 2, ItemPlace::equipment(3), 1);
    inv.apply(&InventoryEvent::ItemAdded {
        item: 3,
        what: ItemRef::Literal("token".into()),
        at: ItemPlace::equipment(5),
        quantity: 1,
    });
    assert_eq!(
        inv.equipped_records(),
        vec![(3, Did(0x7000_0002)), (17, Did(0x7000_0001))]
    );
}

fn rules() -> SlotRules {
    SlotRules {
        equipment_flag: Some(30),
        equipment_keys: [2u32, 3, 4, 17, 18, 19]
            .into_iter()
            .collect::<BTreeSet<_>>(),
    }
}

fn chest() -> Slots {
    Slots {
        compatible: Some(0x4 | 0x1),
        default: Some(0x4 | (1 << 29)),
        precluded: None,
    }
}

fn ctx(place: ItemPlace, quantity: u32, slots: Slots) -> ItemContext {
    ItemContext {
        item: 7,
        place,
        quantity,
        slots,
        target_occupied: false,
        current_target: Some(42),
    }
}

#[test]
fn slot_rules() {
    let r = rules();
    assert!(r.fits(&chest(), 3));
    assert!(!r.fits(&chest(), 2));
    assert!(
        !r.fits(&chest(), 1),
        "key 1 is not an equipment key of the table"
    );
    assert!(!r.fits(&Slots::default(), 3));
    assert_eq!(r.default_equip_key(&chest()), Some(3));
    let two_hands = Slots {
        compatible: Some(0x30000),
        default: Some(0x30000 | (1 << 29)),
        precluded: None,
    };
    assert_eq!(r.default_equip_key(&two_hands), Some(17), "the lowest key");
    let backpack_default = Slots {
        compatible: Some(0x4),
        default: Some(0x4),
        precluded: None,
    };
    assert_eq!(
        r.default_equip_key(&backpack_default),
        None,
        "no Equipment bit"
    );
    let flag_only = Slots {
        compatible: None,
        default: Some(1 << 29),
        precluded: None,
    };
    assert_eq!(r.default_equip_key(&flag_only), None);
    let no_flag = SlotRules {
        equipment_flag: None,
        ..r
    };
    assert_eq!(no_flag.default_equip_key(&chest()), None);
}

#[test]
fn command_choice_section_6() {
    let r = rules();
    let bp = ItemPlace::backpack(4);
    let drop = |target| ItemAction::Drop {
        target,
        split: false,
    };
    // Row 1, 2: paperdoll drops.
    assert_eq!(
        choose_command(&ctx(bp, 1, chest()), drop(DropTarget::PaperdollSlot(3)), &r),
        Some(InventoryCommand::EquipItem { item: 7, slot: 3 })
    );
    assert_eq!(
        choose_command(&ctx(bp, 1, chest()), drop(DropTarget::PaperdollSlot(2)), &r),
        None
    );
    // Row 3, 4: activation.
    assert_eq!(
        choose_command(&ctx(bp, 1, chest()), ItemAction::Activate, &r),
        Some(InventoryCommand::EquipItem { item: 7, slot: 3 })
    );
    assert_eq!(
        choose_command(&ctx(bp, 1, Slots::default()), ItemAction::Activate, &r),
        Some(InventoryCommand::UseItem {
            item: 7,
            target: Some(42)
        })
    );
    // Row 5: equipped item dropped on a backpack slot or the bag window.
    let eq = ItemPlace::equipment(3);
    assert_eq!(
        choose_command(&ctx(eq, 1, chest()), drop(DropTarget::Slot(bp)), &r),
        Some(InventoryCommand::MoveItem { item: 7, to: bp })
    );
    assert_eq!(
        choose_command(&ctx(eq, 1, chest()), drop(DropTarget::BagWindow), &r),
        Some(InventoryCommand::UnequipItem { item: 7 })
    );
    assert_eq!(
        choose_command(&ctx(bp, 1, chest()), drop(DropTarget::BagWindow), &r),
        None
    );
    // Row 6: activating an equipped item.
    assert_eq!(
        choose_command(&ctx(eq, 1, chest()), ItemAction::Activate, &r),
        Some(InventoryCommand::UnequipItem { item: 7 })
    );
    // Row 7: another slot of the same or another bag.
    let other = ItemPlace::backpack(80);
    assert_eq!(
        choose_command(&ctx(bp, 1, chest()), drop(DropTarget::Slot(other)), &r),
        Some(InventoryCommand::MoveItem { item: 7, to: other })
    );
    assert_eq!(
        choose_command(&ctx(bp, 1, chest()), drop(DropTarget::Slot(bp)), &r),
        None,
        "own slot"
    );
    // Row 8: split.
    let split = ItemAction::Drop {
        target: DropTarget::Slot(other),
        split: true,
    };
    assert_eq!(
        choose_command(&ctx(bp, 5, chest()), split, &r),
        Some(InventoryCommand::SplitStack {
            item: 7,
            quantity: 2
        })
    );
    assert_eq!(
        choose_command(&ctx(bp, 1, chest()), split, &r),
        Some(InventoryCommand::MoveItem { item: 7, to: other }),
        "a stack of 1 moves"
    );
    let mut occupied = ctx(bp, 5, chest());
    occupied.target_occupied = true;
    assert_eq!(
        choose_command(&occupied, split, &r),
        Some(InventoryCommand::MoveItem { item: 7, to: other }),
        "split only onto an empty slot"
    );
    // Row 9: destroy.
    assert_eq!(
        choose_command(&ctx(eq, 1, chest()), ItemAction::DestroyConfirmed, &r),
        Some(InventoryCommand::DestroyItem { item: 7 })
    );
    // An equipped item dropped on a paperdoll slot sends nothing.
    assert_eq!(
        choose_command(&ctx(eq, 1, chest()), drop(DropTarget::PaperdollSlot(3)), &r),
        None
    );
}

#[test]
fn inventory_fills_the_command_context() {
    let r = rules();
    let mut inv = Inventory::new(BAGS);
    add(&mut inv, 7, ItemPlace::backpack(0), 6);
    add(&mut inv, 8, ItemPlace::backpack(1), 1);
    let split = |slot| ItemAction::Drop {
        target: DropTarget::Slot(ItemPlace::backpack(slot)),
        split: true,
    };
    assert_eq!(
        inv.choose_command(7, chest(), split(1), &r, None),
        Some(InventoryCommand::MoveItem {
            item: 7,
            to: ItemPlace::backpack(1)
        })
    );
    assert_eq!(
        inv.choose_command(7, chest(), split(2), &r, None),
        Some(InventoryCommand::SplitStack {
            item: 7,
            quantity: 3
        })
    );
    assert_eq!(inv.choose_command(99, chest(), split(2), &r, None), None);
}

fn element(
    id: u32,
    rect: Rect,
    props: Vec<PropertyEntry>,
    children: Vec<ElementDesc>,
) -> ElementDesc {
    let mut e = ElementDesc {
        element_id: id,
        children,
        ..ElementDesc::default()
    };
    e.block.rect = rect;
    e.block.properties = PropertySet {
        entries: props,
        ..PropertySet::default()
    };
    e
}

const P_SLOT: u32 = 0x100;
const P_EMPTY: u32 = 0x101;
const P_LIST: u32 = 0x102;

fn slot_parent(parent: u32, slot: u32, bits: u32, x: i32, y: i32) -> ElementDesc {
    let slot = element(
        slot,
        Rect {
            x: 3,
            y: 3,
            width: 35,
            height: 35,
        },
        vec![PropertyEntry {
            pid: P_SLOT,
            value: PropertyValue::Bitfield32(bits),
        }],
        vec![],
    );
    element(
        parent,
        Rect {
            x,
            y,
            width: 44,
            height: 44,
        },
        vec![],
        vec![slot],
    )
}

fn list(ids: &[u32]) -> PropertyEntry {
    PropertyEntry {
        pid: P_LIST,
        value: PropertyValue::Array(PropertyArray {
            count: u32::try_from(ids.len()).unwrap_or(0),
            elements: ids
                .iter()
                .map(|&i| PropertyEntry {
                    pid: 0x200,
                    value: PropertyValue::Enum(i),
                })
                .collect(),
        }),
    }
}

#[test]
fn paperdoll_table_from_layout() {
    let field = element(
        50,
        Rect::default(),
        vec![list(&[11, 12])],
        vec![
            slot_parent(1, 11, 0x2, 383, 10),
            slot_parent(2, 12, 0x2000, 13, 10),
            // A tooltip-only parent without a slot.
            element(3, Rect::default(), vec![], vec![]),
        ],
    );
    let layout = UiLayout {
        elements: vec![element(49, Rect::default(), vec![], vec![field])],
        ..UiLayout::default()
    };
    let pids = PaperdollPids {
        container_slot: Some(P_SLOT),
        empty_override: Some(P_EMPTY),
        slot_list: Some(P_LIST),
    };
    let t = PaperdollTable::from_layout(&layout, 50, &pids, |id| Some(format!("E{id}")))
        .expect("field found");
    assert_eq!(t.slots.len(), 2);
    assert_eq!(t.listed, vec![11, 12]);
    assert_eq!(t.keys().into_iter().collect::<Vec<_>>(), vec![2, 14]);
    assert_eq!(
        t.slot(2).map(|s| (s.parent, s.position)),
        Some((1, (383, 10)))
    );
    assert_eq!(t.slot(14).and_then(|s| s.label.clone()), Some("E12".into()));
    assert!(t.problems().is_empty());
    assert_eq!(t.rules(Some(30)).equipment_keys.len(), 2);
    assert!(PaperdollTable::from_layout(&layout, 77, &pids, |_| None).is_none());
}

#[test]
fn paperdoll_invariants_cpi1_cpi2() {
    let field = element(
        50,
        Rect::default(),
        vec![list(&[11, 99])],
        vec![
            slot_parent(1, 11, 0x6, 0, 0),
            slot_parent(2, 12, 0x2, 0, 0),
            slot_parent(3, 13, 0x2, 0, 0),
        ],
    );
    let pids = PaperdollPids {
        container_slot: Some(P_SLOT),
        empty_override: None,
        slot_list: Some(P_LIST),
    };
    let t = PaperdollTable::from_slot_field(&field, &pids, |_| None);
    assert_eq!(
        t.problems(),
        vec![
            PaperdollProblem::NotOneBit {
                element: 11,
                bits: 0x6
            },
            PaperdollProblem::DuplicateKey { key: 2 },
            PaperdollProblem::ListedNotSlot { element: 99 },
        ]
    );
    let empty = PaperdollTable::default();
    assert_eq!(empty.problems(), vec![PaperdollProblem::Empty]);
}

#[test]
fn many_events_stay_bounded() {
    let mut inv = Inventory::new(BAGS);
    for i in 0..2_000u64 {
        let slot = u16::try_from(i % 97).unwrap_or(0);
        let ev = match i % 5 {
            0 => InventoryEvent::ItemAdded {
                item: i % 50,
                what: ItemRef::Literal("x".into()),
                at: ItemPlace::backpack(slot),
                quantity: 1,
            },
            1 => InventoryEvent::ItemMoved {
                item: i % 50,
                at: ItemPlace::equipment(slot),
            },
            2 => InventoryEvent::ItemRemoved { item: (i * 7) % 50 },
            3 => InventoryEvent::ContainerSize {
                container: Container::Backpack,
                slots: slot,
            },
            _ => InventoryEvent::ItemQuantity {
                item: i % 50,
                quantity: u32::try_from(i).unwrap_or(0),
            },
        };
        let c = inv.apply(&ev);
        assert!(c.len() <= 3);
        // Every shown item is at its own place.
        for item in inv.items() {
            if let Some(shown) = inv.at(item.place) {
                assert_eq!(shown.place, item.place);
            }
        }
    }
    assert!(inv.len() <= 50);
}

// -----------------------------------------------------------------------------------------
// Review regressions (2026-10-10)
// -----------------------------------------------------------------------------------------

#[test]
fn activation_equips_to_the_mounted_key() {
    // The page has no key 22; `Mounted` + `Equipment` (0x20200000) still equips to 22.
    let r = rules();
    let bridle = Slots {
        compatible: Some(1 << 21),
        default: Some((1 << 21) | (1 << 29)),
        precluded: None,
    };
    assert!(r.is_equipment_key(22));
    assert!(!r.is_equipment_key(26));
    assert_eq!(r.default_equip_key(&bridle), Some(22));
    assert_eq!(
        choose_command(
            &ctx(ItemPlace::backpack(0), 1, bridle),
            ItemAction::Activate,
            &r
        ),
        Some(InventoryCommand::EquipItem { item: 7, slot: 22 })
    );
    assert!(!r.fits(&bridle, 22), "a drop still needs a page slot");
    // A key outside the equipment (here 27) is not chosen.
    let odd = Slots {
        compatible: None,
        default: Some((1 << 26) | (1 << 29)),
        precluded: None,
    };
    assert_eq!(r.default_equip_key(&odd), None);
}

#[test]
fn slot_drop_on_an_equipment_place_is_a_paperdoll_drop() {
    let r = rules();
    let bp = ItemPlace::backpack(4);
    for split in [false, true] {
        let on = |key| ItemAction::Drop {
            target: DropTarget::Slot(ItemPlace::equipment(key)),
            split,
        };
        assert_eq!(
            choose_command(&ctx(bp, 4, Slots::default()), on(2), &r),
            None,
            "an item that fits nowhere is refused (split {split})"
        );
        assert_eq!(
            choose_command(&ctx(bp, 4, chest()), on(2), &r),
            None,
            "chest item on the head slot (split {split})"
        );
        assert_eq!(
            choose_command(&ctx(bp, 4, chest()), on(3), &r),
            Some(InventoryCommand::EquipItem { item: 7, slot: 3 }),
            "split {split}"
        );
        assert_eq!(
            choose_command(&ctx(ItemPlace::equipment(17), 1, chest()), on(3), &r),
            None,
            "an equipped item dropped on a paperdoll slot (split {split})"
        );
    }
}

#[test]
fn offered_bags_does_not_walk_empty_bags() {
    let huge = BagLayout {
        base_slots: 75,
        slots_per_bag: 15,
        bags: u32::MAX,
    };
    let start = std::time::Instant::now();
    let bags = huge.offered_bags(75);
    assert!(start.elapsed() < std::time::Duration::from_secs(1));
    assert_eq!(
        bags,
        vec![
            (1, 0..15),
            (2, 15..30),
            (3, 30..45),
            (4, 45..60),
            (5, 60..75)
        ]
    );
    assert_eq!(huge.offered_bags(0), vec![]);
    // The last bag still shows the remainder when n reaches it.
    let six = BagLayout { bags: 6, ..huge };
    assert_eq!(six.offered_bags(100).last(), Some(&(6, 75..100)));
    assert_eq!(six.offered_bags(75).len(), 5);
    assert_eq!(six.offered_bags(76).len(), 6);
    let zero = BagLayout {
        slots_per_bag: 0,
        bags: 1_000_000,
        ..huge
    };
    assert_eq!(zero.offered_bags(10), vec![(1_000_000, 0..10)]);
    // Same answers as bag_range for every bag of the install's shape.
    for n in 0..120 {
        let brute: Vec<(u32, std::ops::Range<u32>)> = (1..=6)
            .filter_map(|b| six.bag_range(b, n).map(|r| (b, r)))
            .collect();
        assert_eq!(six.offered_bags(n), brute, "n = {n}");
    }
}

#[test]
fn bag_layout_values_are_clamped() {
    let c = InventoryControl {
        base_backpack_slots: Some(u32::MAX),
        slots_per_bag: Some(15),
        max_bags: Some(2_147_483_647),
        ..InventoryControl::default()
    };
    let l = BagLayout::from_control(&c).expect("layout");
    assert_eq!(l.bags, MAX_BAG_VALUE);
    assert_eq!(l.base_slots, MAX_BAG_VALUE);
    assert_eq!(l.slots_per_bag, 15);
}
