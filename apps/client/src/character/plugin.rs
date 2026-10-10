// SPDX-License-Identifier: GPL-3.0-or-later
//! The plugin inventory views' source and events (`lua-plugin-api.md` §15.1): the client's
//! side of `tessera_plugins::InventorySource`.
//!
//! [`PluginInventory`] mirrors the player's inventory model as plugin items
//! ([`InventoryItem`]: the record's `ItemInfo` values, quantity, wear state, icon) and turns
//! each change list of [`PlayerState`] into the §15.1 events, with each item's values
//! taken at the moment of its change (`lua-plugin-api.md` §15: argument values taken at
//! that moment). The getters (`GetSize`, `GetItem`) read the mirror, which follows the
//! model after every change list, so they return the values at delivery.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, PoisonError, RwLock};

use tessera_formats::Did;
use tessera_gamedata::inventory::{
    Container, InventoryChange, ItemId, ItemPlace, ItemRef, PluginEvent, plugin_events,
};
use tessera_gamedata::tooltip::rules::condition_wear_state;
use tessera_net::PropertyValue;
use tessera_plugins::{InventoryItem, InventorySource, InventoryViewEvent, ItemInfoData};

use super::data::{CharacterData, ItemFacts};
use super::state::{PlayerChange, PlayerState};

/// The mirror the plugin thread reads.
#[derive(Debug, Default)]
struct Mirror {
    size: u32,
    items: HashMap<ItemId, Arc<InventoryItem>>,
    backpack: HashMap<u16, ItemId>,
    equipment: HashMap<u16, ItemId>,
}

/// The [`InventorySource`] handed to the plugin manager: a shared view of the mirror.
#[derive(Debug, Clone, Default)]
pub struct InventoryView {
    mirror: Arc<RwLock<Mirror>>,
}

impl InventoryView {
    fn read(&self) -> std::sync::RwLockReadGuard<'_, Mirror> {
        self.mirror.read().unwrap_or_else(PoisonError::into_inner)
    }
}

impl InventorySource for InventoryView {
    fn backpack_size(&self) -> u32 {
        self.read().size
    }

    fn backpack_item(&self, slot: u32) -> Option<Arc<InventoryItem>> {
        let m = self.read();
        if slot >= m.size {
            return None;
        }
        let id = m.backpack.get(&u16::try_from(slot).ok()?)?;
        m.items.get(id).cloned()
    }

    fn equipment_item(&self, key: u32) -> Option<Arc<InventoryItem>> {
        let m = self.read();
        let id = m.equipment.get(&u16::try_from(key).ok()?)?;
        m.items.get(id).cloned()
    }

    fn item(&self, id: u64) -> Option<Arc<InventoryItem>> {
        self.read().items.get(&id).cloned()
    }
}

/// The values of a literal item, shared by every literal item with the same text is not
/// required (§15.1: literal items have an `ItemInfo` of their own per `Item` object).
fn facts_of(data: Option<&CharacterData>, what: &ItemRef) -> Arc<ItemFacts> {
    match what {
        ItemRef::Record(did) => data
            .and_then(|d| d.item(*did))
            .unwrap_or_else(|| Arc::new(unknown_record(*did))),
        ItemRef::Literal(text) => Arc::new(ItemFacts::literal(text)),
    }
}

/// The facts of a record the install does not hold (or without an install): its record
/// DID and nothing else (§15.1 "Absent" column).
fn unknown_record(did: Did) -> ItemFacts {
    let mut f = ItemFacts::literal("");
    f.info = Arc::new(ItemInfoData {
        record: Some(did.0),
        ..ItemInfoData::default()
    });
    f
}

/// The plugin item of model item `id` with its current values.
fn plugin_item(
    state: &PlayerState,
    data: Option<&CharacterData>,
    id: ItemId,
) -> Option<Arc<InventoryItem>> {
    let it = state.inventory().item(id)?;
    let facts = facts_of(data, &it.what);
    let literal = matches!(it.what, ItemRef::Literal(_));
    let wear_state = match it.condition {
        Some((cur, max)) => condition_wear_state(cur, max, facts.wear_state).unwrap_or(0),
        None if literal => 0,
        None => facts.wear_state.unwrap_or(0),
    };
    // §15.1 `Item:GetIconImageID()`: the instance's checkpointed override when it carries
    // one, else `Icon_Layer_ImageDID`.
    let override_pid =
        data.and_then(|d| d.pid(tessera_material::icon::names::IMAGE_OVERRIDE_CHECKPOINTED));
    let icon_override = override_pid.and_then(|p| {
        state
            .item_properties(id)
            .find(|(pid, _)| *pid == p)
            .map(|(_, v)| match v {
                PropertyValue::Did(d) => d.0,
                _ => 0,
            })
    });
    Some(Arc::new(InventoryItem {
        id,
        info: Arc::clone(&facts.info),
        quantity: it.quantity,
        wear_state,
        icon_image_id: if literal {
            0
        } else {
            icon_override.unwrap_or(facts.icon_image)
        },
    }))
}

/// The client's side of the plugin inventory views (module docs).
#[derive(Debug, Default)]
pub struct PluginInventory {
    view: InventoryView,
}

impl PluginInventory {
    /// An empty mirror.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The source to give the plugin manager
    /// (`tessera_plugins::PluginManager::set_inventory_source`).
    #[must_use]
    pub fn source(&self) -> Arc<dyn InventorySource> {
        Arc::new(self.view.clone())
    }

    /// The source as its concrete type (tests).
    #[must_use]
    pub fn view(&self) -> &InventoryView {
        &self.view
    }

    /// Takes one change list of `state` (already applied to it) and returns the plugin
    /// events of §15.1, in order; the mirror then follows the model.
    pub fn update(
        &mut self,
        state: &PlayerState,
        data: Option<&CharacterData>,
        changes: &[PlayerChange],
    ) -> Vec<InventoryViewEvent> {
        let mut out = Vec::new();
        // The values of every item whose facts this list changes, resolved before the
        // plugin thread's readers are locked out (record reads and string renders happen
        // here, once per item).
        let mut fresh: HashMap<ItemId, Arc<InventoryItem>> = HashMap::new();
        let mut seen = HashSet::new();
        for c in changes {
            let id = match c {
                PlayerChange::Inventory(
                    InventoryChange::Added { item, .. } | InventoryChange::Condition { item, .. },
                )
                | PlayerChange::ItemProperty { item, .. } => *item,
                _ => continue,
            };
            if seen.insert(id)
                && let Some(v) = plugin_item(state, data, id)
            {
                fresh.insert(id, v);
            }
        }
        let mut m = self
            .view
            .mirror
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        for c in changes {
            match c {
                PlayerChange::Reset => {
                    // Tessera rule (`lua-plugin-api.md` §15.1 OPEN added by this
                    // implementer): every item the plugins saw leaves its view as if it
                    // were removed (`ItemRemoved` from the backpack, `ItemUnequipped`
                    // from the paperdoll), then `SizeChanged` when *N* changed.
                    let old_size = m.size;
                    let mut gone: Vec<(ItemId, ItemPlace)> = m
                        .backpack
                        .iter()
                        .filter(|(slot, _)| u32::from(**slot) < old_size)
                        .map(|(slot, id)| (*id, ItemPlace::backpack(*slot)))
                        .chain(
                            m.equipment
                                .iter()
                                .map(|(key, id)| (*id, ItemPlace::equipment(*key))),
                        )
                        .collect();
                    gone.sort_by_key(|(_, p)| *p);
                    for (id, from) in gone {
                        let item = m.items.get(&id).cloned();
                        let change = InventoryChange::Removed { item: id, from };
                        for pe in plugin_events(std::slice::from_ref(&change)) {
                            if let Some(ev) = view_event(pe, item.as_ref()) {
                                out.push(ev);
                            }
                        }
                    }
                    *m = Mirror::default();
                    m.size = state.inventory().backpack_size();
                    if m.size != old_size {
                        out.push(InventoryViewEvent::BackpackSizeChanged);
                    }
                }
                PlayerChange::Inventory(ic) => {
                    // The item's values at this change.
                    let item = match ic {
                        InventoryChange::Added { item, .. } => {
                            let v = fresh.get(item).cloned();
                            if let Some(v) = &v {
                                m.items.insert(*item, Arc::clone(v));
                            }
                            v
                        }
                        InventoryChange::Removed { item, .. } => m.items.remove(item),
                        InventoryChange::Moved { item, .. } => m.items.get(item).cloned(),
                        InventoryChange::Quantity { item, new, .. } => {
                            let n = m.items.get(item).map(|v| {
                                let mut n = (**v).clone();
                                n.quantity = *new;
                                Arc::new(n)
                            });
                            if let Some(n) = &n {
                                m.items.insert(*item, Arc::clone(n));
                            }
                            n
                        }
                        InventoryChange::Condition { item, .. } => {
                            let v = fresh.get(item).cloned();
                            if let Some(v) = &v {
                                m.items.insert(*item, Arc::clone(v));
                            }
                            v
                        }
                        InventoryChange::BackpackSize { new, .. } => {
                            m.size = *new;
                            None
                        }
                        InventoryChange::ContainerSize { .. } => None,
                    };
                    for pe in plugin_events(std::slice::from_ref(ic)) {
                        if let Some(ev) = view_event(pe, item.as_ref()) {
                            out.push(ev);
                        }
                    }
                }
                PlayerChange::ItemProperty { item, .. } => {
                    if let Some(v) = fresh.get(item) {
                        m.items.insert(*item, Arc::clone(v));
                    }
                }
                _ => {}
            }
        }
        if changes
            .iter()
            .any(|c| matches!(c, PlayerChange::Inventory(_) | PlayerChange::Reset))
        {
            let inv = state.inventory();
            m.size = inv.backpack_size();
            m.backpack = inv
                .in_container(Container::Backpack)
                .map(|i| (i.place.slot, i.id))
                .collect();
            m.equipment = inv
                .in_container(Container::Equipment)
                .map(|i| (i.place.slot, i.id))
                .collect();
        }
        out
    }
}

impl PluginInventory {
    /// Re-reads the values of every held item from `state` (the install facts arrived
    /// after items did, `data`): the mirror's items, sizes and places follow the model
    /// again. No events (the getters return the new values from now on).
    pub fn rebuild(&mut self, state: &PlayerState, data: Option<&CharacterData>) {
        let inv = state.inventory();
        let items: HashMap<ItemId, Arc<InventoryItem>> = inv
            .items()
            .filter_map(|it| Some((it.id, plugin_item(state, data, it.id)?)))
            .collect();
        let mut m = self
            .view
            .mirror
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        m.items = items;
        m.size = inv.backpack_size();
        m.backpack = inv
            .in_container(Container::Backpack)
            .map(|i| (i.place.slot, i.id))
            .collect();
        m.equipment = inv
            .in_container(Container::Equipment)
            .map(|i| (i.place.slot, i.id))
            .collect();
    }
}

/// A plugin event with its item's values.
fn view_event(pe: PluginEvent, item: Option<&Arc<InventoryItem>>) -> Option<InventoryViewEvent> {
    let it = || item.cloned();
    Some(match pe {
        PluginEvent::BackpackSizeChanged => InventoryViewEvent::BackpackSizeChanged,
        PluginEvent::BackpackItemAdded { index, .. } => {
            InventoryViewEvent::BackpackItemAdded { index, item: it()? }
        }
        PluginEvent::BackpackItemRemoved { index, .. } => {
            InventoryViewEvent::BackpackItemRemoved { index, item: it()? }
        }
        PluginEvent::BackpackItemMoved {
            old_index,
            new_index,
            ..
        } => InventoryViewEvent::BackpackItemMoved {
            old_index,
            new_index,
            item: it()?,
        },
        PluginEvent::ItemEquipped { index, .. } => {
            InventoryViewEvent::ItemEquipped { index, item: it()? }
        }
        PluginEvent::ItemUnequipped { index, .. } => {
            InventoryViewEvent::ItemUnequipped { index, item: it()? }
        }
        PluginEvent::QuantityChanged { .. } => InventoryViewEvent::QuantityChanged { item: it()? },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::state::NO_INSTALL_BAGS;
    use tessera_net::domain::{
        Container as NetContainer, InventoryEvent, ItemId as NetId, ItemPlace, Ref,
    };
    use tessera_net::{EntityId, Event, Transform, Vec3};

    fn run(state: &mut PlayerState, p: &mut PluginInventory, ev: Event) -> Vec<InventoryViewEvent> {
        let mut changes = Vec::new();
        state.apply(&ev, &mut changes);
        p.update(state, None, &changes)
    }

    fn place(container: NetContainer, slot: u16) -> ItemPlace {
        ItemPlace { container, slot }
    }

    #[test]
    fn events_and_getters_follow_the_model() {
        let mut s = PlayerState::new(NO_INSTALL_BAGS);
        let mut p = PluginInventory::new();
        run(
            &mut s,
            &mut p,
            Event::WorldEntered {
                player: EntityId(1),
                name: "Tamsin".into(),
                transform: Transform::new(Vec3::ZERO, 0.0),
            },
        );
        let ev = run(
            &mut s,
            &mut p,
            Event::Inventory(InventoryEvent::ContainerSize {
                container: NetContainer::Backpack,
                slots: 30,
            }),
        );
        assert_eq!(ev, vec![InventoryViewEvent::BackpackSizeChanged]);
        for (id, slot) in [(1, 0), (2, 4), (3, 29)] {
            let ev = run(
                &mut s,
                &mut p,
                Event::Inventory(InventoryEvent::ItemAdded {
                    item: NetId(id),
                    what: Ref::Literal(format!("Thing {id}")),
                    at: place(NetContainer::Backpack, slot),
                    quantity: 1,
                }),
            );
            assert!(matches!(
                ev.as_slice(),
                [InventoryViewEvent::BackpackItemAdded { index, item }]
                    if *index == u32::from(slot) + 1 && item.id == id
            ));
        }
        let src = p.view().clone();
        assert_eq!(src.backpack_size(), 30);
        assert_eq!(src.backpack_item(4).map(|i| i.id), Some(2));
        assert!(src.backpack_item(30).is_none());
        assert_eq!(
            src.backpack_item(29).map(|i| i.info.name.clone()),
            Some("Thing 3".into())
        );
        // Equip at key 3: ItemRemoved from the backpack, then ItemEquipped with index 2.
        let ev = run(
            &mut s,
            &mut p,
            Event::Inventory(InventoryEvent::ItemMoved {
                item: NetId(2),
                at: place(NetContainer::Equipment, 3),
            }),
        );
        assert!(matches!(
            ev.as_slice(),
            [
                InventoryViewEvent::BackpackItemRemoved { index: 5, .. },
                InventoryViewEvent::ItemEquipped { index: 2, .. }
            ]
        ));
        assert_eq!(src.equipment_item(3).map(|i| i.id), Some(2));
        assert!(src.backpack_item(4).is_none());
        // A removed item keeps its last values in the event.
        let ev = run(
            &mut s,
            &mut p,
            Event::Inventory(InventoryEvent::ItemRemoved { item: NetId(3) }),
        );
        assert!(matches!(
            ev.as_slice(),
            [InventoryViewEvent::BackpackItemRemoved { index: 30, item }] if item.id == 3
        ));
        assert!(src.item(3).is_none());
        // Quantity: the event carries the new quantity.
        let ev = run(
            &mut s,
            &mut p,
            Event::Inventory(InventoryEvent::ItemQuantity {
                item: NetId(1),
                quantity: 7,
            }),
        );
        assert!(matches!(
            ev.as_slice(),
            [InventoryViewEvent::QuantityChanged { item }] if item.quantity == 7
        ));
        // A broken item's wear state (literal item with a condition).
        run(
            &mut s,
            &mut p,
            Event::Inventory(InventoryEvent::ItemCondition {
                item: NetId(1),
                current: 0,
                max: 10,
            }),
        );
        assert_eq!(
            src.item(1).map(|i| i.wear_state),
            Some(tessera_gamedata::tooltip::rules::WEAR_BROKEN)
        );
    }

    #[test]
    fn leaving_the_world_removes_every_item_and_keeps_an_unchanged_size_quiet() {
        let mut s = PlayerState::new(NO_INSTALL_BAGS);
        let mut p = PluginInventory::new();
        let enter = || Event::WorldEntered {
            player: EntityId(1),
            name: "Tamsin".into(),
            transform: Transform::new(Vec3::ZERO, 0.0),
        };
        run(&mut s, &mut p, enter());
        for (id, place) in [
            (1, place(NetContainer::Backpack, 0)),
            (2, place(NetContainer::Equipment, 3)),
        ] {
            run(
                &mut s,
                &mut p,
                Event::Inventory(InventoryEvent::ItemAdded {
                    item: NetId(id),
                    what: Ref::Literal(format!("Thing {id}")),
                    at: place,
                    quantity: 1,
                }),
            );
        }
        // Without a ContainerSize, N grew to 1 with the backpack item; after the reset it is
        // 0 again: removals, then SizeChanged.
        let ev = run(
            &mut s,
            &mut p,
            Event::Disconnected {
                reason: tessera_net::DisconnectReason::ClientRequested,
            },
        );
        assert!(
            matches!(
                ev.as_slice(),
                [
                    InventoryViewEvent::BackpackItemRemoved { index: 1, item: a },
                    InventoryViewEvent::ItemUnequipped { index: 2, item: b },
                    InventoryViewEvent::BackpackSizeChanged,
                ] if a.id == 1 && b.id == 2
            ),
            "{ev:?}"
        );
        assert!(p.view().item(1).is_none());
        // Re-entry with nothing held and N unchanged: no event.
        run(&mut s, &mut p, enter());
        let ev = run(
            &mut s,
            &mut p,
            Event::Disconnected {
                reason: tessera_net::DisconnectReason::ClientRequested,
            },
        );
        assert!(ev.is_empty(), "{ev:?}");
    }

    #[test]
    fn a_rebuild_follows_the_model() {
        let mut s = PlayerState::new(NO_INSTALL_BAGS);
        let mut changes = Vec::new();
        s.apply(
            &Event::WorldEntered {
                player: EntityId(1),
                name: "Tamsin".into(),
                transform: Transform::new(Vec3::ZERO, 0.0),
            },
            &mut changes,
        );
        s.apply(
            &Event::Inventory(InventoryEvent::ItemAdded {
                item: NetId(4),
                what: Ref::Literal("Lamp".into()),
                at: place(NetContainer::Backpack, 2),
                quantity: 3,
            }),
            &mut changes,
        );
        // The views never saw the changes (the facts arrived later): a rebuild catches up.
        let mut p = PluginInventory::new();
        p.rebuild(&s, None);
        let src = p.view();
        assert_eq!(src.backpack_size(), 3);
        assert_eq!(
            src.backpack_item(2).map(|i| (i.id, i.quantity)),
            Some((4, 3))
        );
    }
}
