// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data test of the inventory facts read from the install (`character-panels.md`
//! §3.3, §4.2, CPI1–CPI3) against a user's own install, read-only.
//!
//! Skipped (reported as passed) unless `TESSERA_GAME_DIR` points at an install. Prints
//! counts and keys only. Values are read from the install at run time.

use std::path::PathBuf;

use tessera_gamedata::inventory::{
    BACKPACK_SLOT_LABEL, BagLayout, Inventory, InventoryEvent, ItemPlace, ItemRef, PluginEvent,
    api_equipment_index, check_api_names, equipment_flag_key, load_inventory_control,
    load_paperdoll, slot_label_key, slot_names,
};
use tessera_wstate::ObjectStore;

fn game_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("TESSERA_GAME_DIR")?);
    dir.is_dir().then_some(dir)
}

#[test]
fn inventory_facts_from_the_install() {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping the inventory real-data test");
        return;
    };
    let store = ObjectStore::open(&dir).expect("install opens");

    // §4.2.
    let control = load_inventory_control(&store).expect("InventoryControl");
    eprintln!("InventoryControl: {control:?}");
    let bags = BagLayout::from_control(&control).expect("bag values present");
    assert!(bags.bags >= 1 && bags.slots_per_bag >= 1 && bags.base_slots >= 1);
    let offered = bags.offered_bags(bags.base_slots);
    assert!(!offered.is_empty());
    let shown: u32 = offered.iter().map(|(_, r)| r.end - r.start).sum();
    assert_eq!(
        shown, bags.base_slots,
        "every base slot is in exactly one bag"
    );

    // §3.3, CPI1–CPI2.
    let table = load_paperdoll(&store).expect("paperdoll slot table");
    let problems = table.problems();
    assert!(problems.is_empty(), "CPI1/CPI2: {problems:?}");
    let keys = table.keys();
    eprintln!(
        "paperdoll: {} slots, {} listed, keys {:?}",
        table.slots.len(),
        table.listed.len(),
        keys
    );
    for s in &table.slots {
        assert!(
            s.empty_tooltip.is_some(),
            "slot {:?} has an empty tooltip",
            s.label
        );
    }

    // CPI3 and the special keys.
    let names = slot_names(&store).expect("slot names");
    let failures = check_api_names(|k| names.get(&k).cloned());
    assert!(failures.is_empty(), "CPI3: {failures:?}");
    let flag = equipment_flag_key(&store)
        .expect("dictionary")
        .expect("Equipment label");
    let backpack = slot_label_key(&store, BACKPACK_SLOT_LABEL)
        .expect("dictionary")
        .expect("Backpack label");
    assert!(!keys.contains(&flag) && !keys.contains(&backpack));
    let rules = table.rules(Some(flag));
    assert_eq!(rules.equipment_keys, keys);
    let api_visible = keys
        .iter()
        .filter(|k| api_equipment_index(**k).is_some())
        .count();
    eprintln!(
        "Equipment flag key {flag}, Backpack key {backpack}; {api_visible} slots visible to plugins"
    );

    // Check 66, second half: growth before the first size from the install's base.
    let mut inv = Inventory::new(bags);
    let b = u16::try_from(bags.base_slots).expect("base fits a slot");
    let ev = inv.apply_for_plugins(&InventoryEvent::ItemAdded {
        item: 1,
        what: ItemRef::Literal("probe".into()),
        at: ItemPlace::backpack(b + 5),
        quantity: 1,
    });
    assert_eq!(
        ev,
        vec![
            PluginEvent::BackpackSizeChanged,
            PluginEvent::BackpackItemAdded {
                index: u32::from(b) + 6,
                item: 1
            }
        ]
    );
    assert_eq!(inv.backpack_size(), bags.base_slots + 6);
}
