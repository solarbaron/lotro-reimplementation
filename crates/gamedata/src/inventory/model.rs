// SPDX-License-Identifier: GPL-3.0-or-later
//! The local player's containers as the session's inventory events fill them
//! (`character-panels.md` §4.2, §4.3, §9; `scenario-feed.md` §5.5, §9.2), the change list
//! each event yields, and the plugin events derived from it (`lua-plugin-api.md` §15.1).
//!
//! The model is independent of `tessera-net`: the client adapts the session's events into
//! [`InventoryEvent`]s. It changes only on events, never on the player's commands
//! (`character-panels.md` §6).

use std::collections::BTreeMap;
use std::ops::Range;

use tessera_formats::Did;

use super::slots::{
    api_equipment_index, backpack_index, key_of_api_equipment_index, slot_of_backpack_index,
};

/// An item instance ID (the session's `ItemId`, `scenario-feed.md` §5.3).
pub type ItemId = u64;

/// A container of the local player (`scenario-feed.md` §9.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Container {
    /// The backpack (bags).
    Backpack,
    /// The equipment; its slots are data `ContainerSlot` keys.
    Equipment,
    /// The vault.
    Vault,
    /// Shared storage.
    SharedStorage,
}

/// A place in a container: the container and a slot (0-based; equipment: the data key).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ItemPlace {
    /// The container.
    pub container: Container,
    /// The slot.
    pub slot: u16,
}

impl ItemPlace {
    /// A backpack slot.
    #[must_use]
    pub const fn backpack(slot: u16) -> Self {
        Self {
            container: Container::Backpack,
            slot,
        }
    }

    /// An equipment slot by data key.
    #[must_use]
    pub const fn equipment(key: u16) -> Self {
        Self {
            container: Container::Equipment,
            slot: key,
        }
    }
}

/// What an item is: a data record or a literal (scripted) item (`scenario-feed.md` §5.2).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ItemRef {
    /// An item record.
    Record(Did),
    /// A literal item with this text.
    Literal(String),
}

/// One item of the model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// Its ID.
    pub id: ItemId,
    /// What it is.
    pub what: ItemRef,
    /// Where it is.
    pub place: ItemPlace,
    /// Its quantity (the last `ItemQuantity`, else the arriving quantity).
    pub quantity: u32,
    /// The last `ItemCondition`: (current, max).
    pub condition: Option<(u32, u32)>,
}

/// An inventory event of the session (`scenario-feed.md` §9.2, Inventory group without
/// the wallet and item properties).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InventoryEvent {
    /// A container's size in slots.
    ContainerSize {
        /// The container.
        container: Container,
        /// Its size.
        slots: u16,
    },
    /// An item enters a container.
    ItemAdded {
        /// The item.
        item: ItemId,
        /// What it is.
        what: ItemRef,
        /// Where.
        at: ItemPlace,
        /// Its quantity.
        quantity: u32,
    },
    /// An item moves (within or between containers).
    ItemMoved {
        /// The item.
        item: ItemId,
        /// Its new place.
        at: ItemPlace,
    },
    /// An item's quantity changes.
    ItemQuantity {
        /// The item.
        item: ItemId,
        /// The new quantity.
        quantity: u32,
    },
    /// An item leaves the containers.
    ItemRemoved {
        /// The item.
        item: ItemId,
    },
    /// An item's condition (durability).
    ItemCondition {
        /// The item.
        item: ItemId,
        /// Current points.
        current: u32,
        /// Maximum points.
        max: u32,
    },
}

/// One change of the model, in the order it happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InventoryChange {
    /// A container's size was set (`old` none: the first size of that container).
    ContainerSize {
        /// The container.
        container: Container,
        /// The previous size.
        old: Option<u16>,
        /// The new size.
        new: u16,
    },
    /// The backpack size *N* of `character-panels.md` §4.3 took a new value.
    BackpackSize {
        /// The previous *N*.
        old: u32,
        /// The new *N*.
        new: u32,
    },
    /// An item entered a container.
    Added {
        /// The item.
        item: ItemId,
        /// Where.
        at: ItemPlace,
    },
    /// An item left a container (removed from the inventory).
    Removed {
        /// The item.
        item: ItemId,
        /// Where it was.
        from: ItemPlace,
    },
    /// An item moved.
    Moved {
        /// The item.
        item: ItemId,
        /// Where it was.
        from: ItemPlace,
        /// Where it is.
        to: ItemPlace,
    },
    /// An item's quantity changed.
    Quantity {
        /// The item.
        item: ItemId,
        /// The previous quantity.
        old: u32,
        /// The new quantity.
        new: u32,
    },
    /// An item's condition was reported.
    Condition {
        /// The item.
        item: ItemId,
        /// Current points.
        current: u32,
        /// Maximum points.
        max: u32,
    },
}

/// A plugin event of `lua-plugin-api.md` §15.1 (before queueing; indices 1-based).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginEvent {
    /// `Backpack.SizeChanged`.
    BackpackSizeChanged,
    /// `Backpack.ItemAdded { Index, Item }`.
    BackpackItemAdded {
        /// The index (slot + 1).
        index: u32,
        /// The item.
        item: ItemId,
    },
    /// `Backpack.ItemRemoved { Index, Item }`.
    BackpackItemRemoved {
        /// The index (slot + 1).
        index: u32,
        /// The item.
        item: ItemId,
    },
    /// `Backpack.ItemMoved { OldIndex, NewIndex, Item }`.
    BackpackItemMoved {
        /// The old index.
        old_index: u32,
        /// The new index.
        new_index: u32,
        /// The item.
        item: ItemId,
    },
    /// `Equipment.ItemEquipped { Index = key − 1, Item }`.
    ItemEquipped {
        /// The API slot value.
        index: u32,
        /// The item.
        item: ItemId,
    },
    /// `Equipment.ItemUnequipped { Index, Item }`.
    ItemUnequipped {
        /// The API slot value.
        index: u32,
        /// The item.
        item: ItemId,
    },
    /// `Item.QuantityChanged` of an item.
    QuantityChanged {
        /// The item.
        item: ItemId,
    },
}

/// The plugin events of one change list (`lua-plugin-api.md` §15.1), in order.
///
/// Per change: `BackpackSize` → `SizeChanged`; an item entering the backpack →
/// `ItemAdded`, leaving it → `ItemRemoved`, a move within it → `ItemMoved`; an item
/// entering the equipment at a key 2 … 22 → `ItemEquipped`, leaving such a key →
/// `ItemUnequipped`; a quantity change → `QuantityChanged`. A move between containers
/// gives the leaving event first, then the entering one; a move within the equipment
/// between API keys gives `ItemUnequipped` then `ItemEquipped` (Tessera rules, OPEN in
/// `lua-plugin-api.md` §15.1). When an arriving item raises *N* (`character-panels.md`
/// §4.3 rule 1), `SizeChanged` comes first, before every event of that arrival.
#[must_use]
pub fn plugin_events(changes: &[InventoryChange]) -> Vec<PluginEvent> {
    let mut out = Vec::new();
    for c in changes {
        match *c {
            InventoryChange::BackpackSize { old, new } if old != new => {
                out.push(PluginEvent::BackpackSizeChanged);
            }
            InventoryChange::Added { item, at } => entered(&mut out, item, at),
            InventoryChange::Removed { item, from } => left(&mut out, item, from),
            InventoryChange::Moved { item, from, to } => {
                if from.container == Container::Backpack && to.container == Container::Backpack {
                    if from.slot != to.slot {
                        out.push(PluginEvent::BackpackItemMoved {
                            old_index: backpack_index(from.slot),
                            new_index: backpack_index(to.slot),
                            item,
                        });
                    }
                } else if from != to {
                    left(&mut out, item, from);
                    entered(&mut out, item, to);
                }
            }
            InventoryChange::Quantity { item, old, new } if old != new => {
                out.push(PluginEvent::QuantityChanged { item });
            }
            _ => {}
        }
    }
    out
}

fn entered(out: &mut Vec<PluginEvent>, item: ItemId, at: ItemPlace) {
    match at.container {
        Container::Backpack => out.push(PluginEvent::BackpackItemAdded {
            index: backpack_index(at.slot),
            item,
        }),
        Container::Equipment => {
            if let Some(index) = api_equipment_index(u32::from(at.slot)) {
                out.push(PluginEvent::ItemEquipped { index, item });
            }
        }
        Container::Vault | Container::SharedStorage => {}
    }
}

fn left(out: &mut Vec<PluginEvent>, item: ItemId, from: ItemPlace) {
    match from.container {
        Container::Backpack => out.push(PluginEvent::BackpackItemRemoved {
            index: backpack_index(from.slot),
            item,
        }),
        Container::Equipment => {
            if let Some(index) = api_equipment_index(u32::from(from.slot)) {
                out.push(PluginEvent::ItemUnequipped { index, item });
            }
        }
        Container::Vault | Container::SharedStorage => {}
    }
}

// -----------------------------------------------------------------------------------------
// Backpack size and bag ranges (§4.2, §4.3)
// -----------------------------------------------------------------------------------------

/// The `InventoryControl` values of `character-panels.md` §4.2, read at run time
/// ([`super::load_inventory_control`]). Absent or negative values read as `None`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InventoryControl {
    /// `InventoryControl_BaseBackpackSlots`.
    pub base_backpack_slots: Option<u32>,
    /// `InventoryControl_SlotsPerBag`.
    pub slots_per_bag: Option<u32>,
    /// `InventoryControl_MaxNumBags`.
    pub max_bags: Option<u32>,
    /// `InventoryControl_SlotsPerPouch`.
    pub slots_per_pouch: Option<u32>,
    /// `InventoryControl_NumEquipmentSlots`.
    pub equipment_slots: Option<u32>,
}

/// How backpack slots are shown in the bag windows (`character-panels.md` §4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BagLayout {
    /// The size *N* starts at this before the first `ContainerSize`.
    pub base_slots: u32,
    /// Slots per bag window (bags 1 … `bags` − 1).
    pub slots_per_bag: u32,
    /// Number of bag windows; the last one shows every slot from
    /// `slots_per_bag` × (`bags` − 1) up.
    pub bags: u32,
}

/// Most bag windows, slots per bag and base slots [`BagLayout::from_control`] keeps
/// (hostile-input bound: a backpack has at most 65,536 slots, `ContainerSize` being a
/// `u16`; the data has 6 bags of 15 slots and 45 base slots).
pub const MAX_BAG_VALUE: u32 = 65_536;

impl BagLayout {
    /// The layout of an `InventoryControl`; `None` when one of the three values is
    /// missing. Each value is clamped to [`MAX_BAG_VALUE`] (logged).
    #[must_use]
    pub fn from_control(c: &InventoryControl) -> Option<Self> {
        let clamp = |what: &str, v: u32| {
            if v > MAX_BAG_VALUE {
                tracing::warn!(value = v, "InventoryControl {what} too large; clamped");
            }
            v.min(MAX_BAG_VALUE)
        };
        Some(Self {
            base_slots: clamp("base backpack slots", c.base_backpack_slots?),
            slots_per_bag: clamp("slots per bag", c.slots_per_bag?),
            bags: clamp("bag count", c.max_bags?),
        })
    }

    /// The slots bag `bag` (1-based) shows for a backpack of size `n`, or `None` when the
    /// bag is not offered (its range is empty, or there is no such bag) (§4.3 rules 2–4).
    #[must_use]
    pub fn bag_range(&self, bag: u32, n: u32) -> Option<Range<u32>> {
        if bag == 0 || bag > self.bags {
            return None;
        }
        let start = self.slots_per_bag.saturating_mul(bag - 1);
        let end = if bag == self.bags {
            n
        } else {
            self.slots_per_bag.saturating_mul(bag).min(n)
        };
        (start < end).then_some(start..end)
    }

    /// The offered bags (1-based) and their ranges, in bag order. The time taken grows
    /// with the number of offered bags, not with [`BagLayout::bags`]: the bags before the
    /// last stop at the first one that starts at or after `n`.
    #[must_use]
    pub fn offered_bags(&self, n: u32) -> Vec<(u32, Range<u32>)> {
        let mut out = Vec::new();
        if self.bags == 0 {
            return out;
        }
        for b in 1..self.bags {
            // Every later bag but the last starts later still, so none is offered.
            if self.slots_per_bag == 0 || self.slots_per_bag.saturating_mul(b - 1) >= n {
                break;
            }
            if let Some(r) = self.bag_range(b, n) {
                out.push((b, r));
            }
        }
        if let Some(r) = self.bag_range(self.bags, n) {
            out.push((self.bags, r));
        }
        out
    }

    /// The bag that shows backpack slot `slot` for size `n`.
    #[must_use]
    pub fn bag_of_slot(&self, slot: u32, n: u32) -> Option<u32> {
        if slot >= n || self.bags == 0 {
            return None;
        }
        if self.slots_per_bag == 0 {
            return Some(self.bags);
        }
        Some((slot / self.slots_per_bag + 1).min(self.bags))
    }
}

// -----------------------------------------------------------------------------------------
// The model
// -----------------------------------------------------------------------------------------

/// Most items the model keeps (hostile-input bound: four containers of 65,536 slots).
pub const MAX_ITEMS: usize = 4 * 65_536;

/// The local player's inventory.
#[derive(Debug, Clone)]
pub struct Inventory {
    bags: BagLayout,
    /// The items and the arrival number of each at its current place.
    items: BTreeMap<ItemId, (Item, u64)>,
    /// (place, arrival number) → item. During a swap two items share a place for one
    /// event; the later arrival is shown and the earlier keeps its place until it moves.
    places: BTreeMap<(ItemPlace, u64), ItemId>,
    arrivals: u64,
    sizes: BTreeMap<Container, u16>,
    /// The backpack size *N*.
    backpack_size: u32,
}

impl Inventory {
    /// An empty inventory whose backpack size starts at `bags.base_slots`.
    #[must_use]
    pub fn new(bags: BagLayout) -> Self {
        Self {
            bags,
            items: BTreeMap::new(),
            places: BTreeMap::new(),
            arrivals: 0,
            sizes: BTreeMap::new(),
            backpack_size: bags.base_slots,
        }
    }

    /// The bag layout.
    #[must_use]
    pub fn bags(&self) -> &BagLayout {
        &self.bags
    }

    /// The backpack size *N* (`Backpack:GetSize()`).
    #[must_use]
    pub fn backpack_size(&self) -> u32 {
        self.backpack_size
    }

    /// The last `ContainerSize` of `container`, if any.
    #[must_use]
    pub fn container_size(&self, container: Container) -> Option<u16> {
        self.sizes.get(&container).copied()
    }

    /// The offered bags and their ranges for the current size.
    #[must_use]
    pub fn offered_bags(&self) -> Vec<(u32, Range<u32>)> {
        self.bags.offered_bags(self.backpack_size)
    }

    /// The item `id`.
    #[must_use]
    pub fn item(&self, id: ItemId) -> Option<&Item> {
        self.items.get(&id).map(|(i, _)| i)
    }

    /// Every item, ascending ID.
    pub fn items(&self) -> impl Iterator<Item = &Item> {
        self.items.values().map(|(i, _)| i)
    }

    /// Number of items.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the inventory holds no item.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The item shown at `place`.
    #[must_use]
    pub fn at(&self, place: ItemPlace) -> Option<&Item> {
        self.places
            .range((place, 0)..=(place, u64::MAX))
            .next_back()
            .and_then(|(_, id)| self.item(*id))
    }

    /// The items shown in `container` by slot, ascending (one per slot).
    pub fn in_container(&self, container: Container) -> impl Iterator<Item = &Item> {
        let lo = ItemPlace { container, slot: 0 };
        let hi = ItemPlace {
            container,
            slot: u16::MAX,
        };
        let mut shown: Vec<&Item> = Vec::new();
        for ((place, _), id) in self.places.range((lo, 0)..=(hi, u64::MAX)) {
            let Some(item) = self.item(*id) else { continue };
            match shown.last_mut() {
                Some(last) if last.place == *place => *last = item,
                _ => shown.push(item),
            }
        }
        shown.into_iter()
    }

    /// The equipped items in ascending key order.
    pub fn equipped(&self) -> impl Iterator<Item = &Item> {
        self.in_container(Container::Equipment)
    }

    /// `Backpack:GetItem(index)` (`lua-plugin-api.md` §15.1): the item at slot
    /// `index` − 1 for an integer `index` in 1 … *N*.
    #[must_use]
    pub fn backpack_item_at_index(&self, index: f64) -> Option<&Item> {
        let slot = slot_of_backpack_index(index, self.backpack_size)?;
        self.at(ItemPlace::backpack(slot))
    }

    /// `Equipment:GetItem(slot)`: the item at data key `slot` + 1 for an integer `slot`
    /// in 1 … 21.
    #[must_use]
    pub fn equipment_item_at_index(&self, slot: f64) -> Option<&Item> {
        let key = key_of_api_equipment_index(slot)?;
        self.at(ItemPlace::equipment(u16::try_from(key).ok()?))
    }

    /// Places `item` (already in `items`) at `place` as the latest arrival there.
    fn place_item(&mut self, id: ItemId, place: ItemPlace) {
        self.arrivals = self.arrivals.wrapping_add(1);
        let seq = self.arrivals;
        if let Some((item, s)) = self.items.get_mut(&id) {
            // Arrival numbers are unique, so this entry is the item's own.
            self.places.remove(&(item.place, *s));
            item.place = place;
            *s = seq;
            self.places.insert((place, seq), id);
        }
    }

    /// Before the first backpack `ContainerSize`, an arrival at slot *s* ≥ *N* raises *N*
    /// to *s* + 1 (§4.3 rule 1).
    fn grow_for(&mut self, at: ItemPlace, out: &mut Vec<InventoryChange>) {
        if at.container != Container::Backpack || self.sizes.contains_key(&Container::Backpack) {
            return;
        }
        let needed = u32::from(at.slot) + 1;
        if needed > self.backpack_size {
            out.push(InventoryChange::BackpackSize {
                old: self.backpack_size,
                new: needed,
            });
            self.backpack_size = needed;
        }
    }

    /// Applies one event and returns its changes, in order. An event that names an
    /// unknown item, an item that already exists (`ItemAdded`), or that would exceed
    /// [`MAX_ITEMS`] changes nothing (logged).
    pub fn apply(&mut self, event: &InventoryEvent) -> Vec<InventoryChange> {
        let mut out = Vec::new();
        match event {
            InventoryEvent::ContainerSize { container, slots } => {
                let old = self.sizes.insert(*container, *slots);
                if old != Some(*slots) {
                    out.push(InventoryChange::ContainerSize {
                        container: *container,
                        old,
                        new: *slots,
                    });
                }
                if *container == Container::Backpack {
                    let new = u32::from(*slots);
                    if new != self.backpack_size {
                        out.push(InventoryChange::BackpackSize {
                            old: self.backpack_size,
                            new,
                        });
                        self.backpack_size = new;
                    }
                }
            }
            InventoryEvent::ItemAdded {
                item,
                what,
                at,
                quantity,
            } => {
                if self.items.contains_key(item) {
                    tracing::warn!(item, "ItemAdded for an item already held; ignored");
                    return out;
                }
                if self.items.len() >= MAX_ITEMS {
                    tracing::warn!(item, "inventory item limit reached; ItemAdded ignored");
                    return out;
                }
                self.grow_for(*at, &mut out);
                self.arrivals = self.arrivals.wrapping_add(1);
                let seq = self.arrivals;
                self.items.insert(
                    *item,
                    (
                        Item {
                            id: *item,
                            what: what.clone(),
                            place: *at,
                            quantity: *quantity,
                            condition: None,
                        },
                        seq,
                    ),
                );
                self.places.insert((*at, seq), *item);
                out.push(InventoryChange::Added {
                    item: *item,
                    at: *at,
                });
            }
            InventoryEvent::ItemMoved { item, at } => {
                let Some(from) = self.item(*item).map(|i| i.place) else {
                    tracing::warn!(item, "ItemMoved for an unknown item; ignored");
                    return out;
                };
                self.grow_for(*at, &mut out);
                self.place_item(*item, *at);
                out.push(InventoryChange::Moved {
                    item: *item,
                    from,
                    to: *at,
                });
            }
            InventoryEvent::ItemQuantity { item, quantity } => {
                let Some((i, _)) = self.items.get_mut(item) else {
                    tracing::warn!(item, "ItemQuantity for an unknown item; ignored");
                    return out;
                };
                let old = i.quantity;
                i.quantity = *quantity;
                out.push(InventoryChange::Quantity {
                    item: *item,
                    old,
                    new: *quantity,
                });
            }
            InventoryEvent::ItemRemoved { item } => {
                let Some((removed, seq)) = self.items.remove(item) else {
                    tracing::warn!(item, "ItemRemoved for an unknown item; ignored");
                    return out;
                };
                self.places.remove(&(removed.place, seq));
                out.push(InventoryChange::Removed {
                    item: *item,
                    from: removed.place,
                });
            }
            InventoryEvent::ItemCondition { item, current, max } => {
                let Some((i, _)) = self.items.get_mut(item) else {
                    tracing::warn!(item, "ItemCondition for an unknown item; ignored");
                    return out;
                };
                i.condition = Some((*current, *max));
                out.push(InventoryChange::Condition {
                    item: *item,
                    current: *current,
                    max: *max,
                });
            }
        }
        out
    }

    /// Applies an event and returns its plugin events ([`plugin_events`]).
    pub fn apply_for_plugins(&mut self, event: &InventoryEvent) -> Vec<PluginEvent> {
        plugin_events(&self.apply(event))
    }

    /// The equipped item records in ascending key order with their keys (the worn-look
    /// list of `character-panels.md` §7; literal items left out).
    #[must_use]
    pub fn equipped_records(&self) -> Vec<(u16, Did)> {
        self.equipped()
            .filter_map(|i| match i.what {
                ItemRef::Record(d) => Some((i.place.slot, d)),
                ItemRef::Literal(_) => None,
            })
            .collect()
    }
}
