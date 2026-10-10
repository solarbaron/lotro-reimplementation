// SPDX-License-Identifier: GPL-3.0-or-later
//! The local player's inventory model (decision 0024 P1): backpack and equipment
//! containers as the session's events fill them, the backpack size and bag ranges, slot ↔
//! container-key mapping and slot compatibility, independent of `tessera-net` (the client
//! adapts its events).
//!
//! Spec: `docs/specs/character-panels.md` §3.3, §4, §6, §9; `docs/specs/items.md`;
//! `docs/specs/lua-plugin-api.md` §15.1.
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`Inventory`], [`InventoryEvent`], [`InventoryChange`], [`Item`], [`ItemPlace`], [`Container`] | The model, the events that feed it and the change list each event yields | `character-panels.md` §9; `scenario-feed.md` §9.2 |
//! | [`InventoryControl`], [`BagLayout`], [`load_inventory_control`] | Backpack size *N* and the slots each bag window shows | `character-panels.md` §4.2, §4.3 |
//! | [`PaperdollTable`], [`PaperdollSlot`], [`PaperdollProblem`], [`load_paperdoll`] | The equipment page's slot table and CPI1–CPI2 | §3.3, §10 |
//! | [`slot_bit`], [`slot_keys`], [`SlotRules`], [`equipment_flag_key`] | `ContainerSlot` keys and bits, slot compatibility | §3.3, §6; `items.md` §5.4 |
//! | [`choose_command`], [`ItemAction`], [`InventoryCommand`] | The command of each interaction of §6 | §6 |
//! | [`plugin_events`], [`PluginEvent`], [`api_equipment_index`], [`backpack_index`], [`check_api_names`] | Plugin indices, plugin events and CPI3 | `lua-plugin-api.md` §15.1; §8, §10 |
//!
//! # Hostile input
//!
//! The model never panics on an event stream: an event naming an unknown item, a second
//! `ItemAdded` of one ID, or one past [`MAX_ITEMS`] items changes nothing and is logged;
//! each event yields at most three changes. Layout reading is bounded by
//! [`MAX_PAPERDOLL_SLOTS`]; the bag layout's values by [`MAX_BAG_VALUE`].

mod commands;
mod load;
mod model;
mod slots;

#[cfg(test)]
mod tests;

pub use commands::{DropTarget, InventoryCommand, ItemAction, ItemContext, choose_command};
pub use load::{
    EQUIPMENT_PAGE_LAYOUT_LABEL, INVENTORY_CONTROL_LABEL, SLOT_FIELD_LABEL, equipment_flag_key,
    load_inventory_control, load_paperdoll, slot_label_key, slot_names,
};
pub use model::{
    BagLayout, Container, Inventory, InventoryChange, InventoryControl, InventoryEvent, Item,
    ItemId, ItemPlace, ItemRef, MAX_BAG_VALUE, MAX_ITEMS, PluginEvent, plugin_events,
};
pub use slots::{
    API_EQUIPMENT_NAMES, API_EQUIPMENT_SIZE, BACKPACK_SLOT_LABEL, EQUIPMENT_FLAG_LABEL,
    FIRST_API_EQUIPMENT_KEY, LAST_API_EQUIPMENT_KEY, MAX_PAPERDOLL_SLOTS, MAX_SLOT_KEY,
    OVERFLOW_SLOT_LABEL, PaperdollPids, PaperdollProblem, PaperdollSlot, PaperdollTable, SlotRules,
    api_equipment_index, backpack_index, check_api_names, has_slot_key, key_of_api_equipment_index,
    single_slot_key, slot_bit, slot_keys, slot_of_backpack_index,
};

/// The item slot bit fields of a record (`items.md` §5.4), re-exported from
/// `tessera-formats`.
pub use tessera_formats::gameplay::items::Slots;
